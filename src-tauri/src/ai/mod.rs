use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, LazyLock};
use std::time::{Duration, Instant};
use tauri::Manager;
use tokio::sync::Mutex;

use crate::config::Config;

pub mod hybrid;
pub mod languagetool;
pub mod local;
pub mod mistral;

// ── Clients HTTP partagés ─────────────────────────────────────────────────────
//
// Un `reqwest::Client` porte son propre pool de connexions : en construire un
// par requête refait le handshake TLS complet à chaque déclenchement. Les
// clients ci-dessous sont clonés par les providers — un clone partage le pool.
//
// Ils portent surtout un **timeout** : sans lui (défaut reqwest = infini) une
// requête bloquée laisse la popup tourner indéfiniment.

const AI_TIMEOUT:    Duration = Duration::from_secs(60); // Ollama à froid ≈ 20 s
const LT_TIMEOUT:    Duration = Duration::from_secs(20);
const PROBE_TIMEOUT: Duration = Duration::from_secs(3);

static AI_CLIENT: LazyLock<reqwest::Client> = LazyLock::new(|| {
    reqwest::Client::builder().timeout(AI_TIMEOUT).build().unwrap_or_default()
});
static LT_CLIENT: LazyLock<reqwest::Client> = LazyLock::new(|| {
    reqwest::Client::builder().timeout(LT_TIMEOUT).build().unwrap_or_default()
});
static PROBE_CLIENT: LazyLock<reqwest::Client> = LazyLock::new(|| {
    reqwest::Client::builder().timeout(PROBE_TIMEOUT).build().unwrap_or_default()
});

pub fn ai_client()    -> reqwest::Client { AI_CLIENT.clone() }
pub fn lt_client()    -> reqwest::Client { LT_CLIENT.clone() }
pub fn probe_client() -> reqwest::Client { PROBE_CLIENT.clone() }

// ── Génération : ignore les résultats périmés ─────────────────────────────────
//
// Chaque déclenchement incrémente le compteur. Un résultat lent qui arrive
// après une nouvelle capture est jeté au lieu d'écraser l'affichage courant.

static GENERATION: AtomicU64 = AtomicU64::new(0);

pub fn next_generation() -> u64 {
    GENERATION.fetch_add(1, Ordering::SeqCst) + 1
}

fn is_current(gen: u64) -> bool {
    GENERATION.load(Ordering::SeqCst) == gen
}

/// Émet vers la popup uniquement si `gen` correspond toujours à la capture courante.
pub fn emit<S: Serialize + Clone>(window: &tauri::Window, gen: u64, event: &str, payload: S) {
    if is_current(gen) {
        let _ = window.emit(event, payload);
    }
}

// ── Result types ─────────────────────────────────────────────────────────────

/// Emitted as soon as LanguageTool finishes (before the AI reformulations).
#[derive(Clone, Serialize)]
pub struct PartialResult {
    pub correction: String,
}

/// `serde(default)` : un modèle qui omet une clé ne doit pas faire échouer tout
/// le parsing — on préfère une reformulation vide à une erreur totale.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AiResult {
    pub correction:   String,
    pub simple:       String,
    pub professional: String,
    pub formal:       String,
    pub short:        String,
    pub creative:     String,
}

impl Default for AiResult {
    fn default() -> Self {
        Self {
            correction:   String::new(),
            simple:       String::new(),
            professional: String::new(),
            formal:       String::new(),
            short:        String::new(),
            creative:     String::new(),
        }
    }
}

impl AiResult {
    /// Résultat « correction seule » : les 5 reformulations portent le même message.
    fn correction_only(correction: String, note: &str) -> Self {
        Self {
            correction,
            simple:       note.to_string(),
            professional: note.to_string(),
            formal:       note.to_string(),
            short:        note.to_string(),
            creative:     note.to_string(),
        }
    }
}

// ── Provider trait ────────────────────────────────────────────────────────────

#[async_trait]
pub trait AiProvider: Send + Sync {
    async fn process(&self, text: &str) -> Result<AiResult>;
}

// ── Fallback chain : détection automatique du provider ─────────────────────────

/// Provider effectivement actif quand `ai_provider == "auto"`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderKind {
    Mistral,
    Ollama,
    LtOnly,
}

/// Délai avant de retenter le haut de la chaîne après une rétrogradation.
/// Sans ça, une coupure réseau d'une seconde dégradait l'app pour toute la session.
const RECOVERY_AFTER: Duration = Duration::from_secs(90);

#[derive(Default)]
pub struct AutoState {
    /// `None` tant que la détection initiale n'a pas abouti.
    pub kind:          Option<ProviderKind>,
    /// Instant de la dernière rétrogradation (base du délai de reprise).
    pub downgraded_at: Option<Instant>,
}

/// État Tauri : provider courant en mode auto, modifiable à chaud (fallback runtime).
pub struct ActiveProvider(pub Arc<Mutex<AutoState>>);

/// Chaîne de fallback : Mistral API → Ollama local → LanguageTool seul.
pub async fn auto_detect_provider(config: &Config) -> ProviderKind {
    if !config.mistral_api_key.is_empty() {
        let m = mistral::MistralProvider::new(
            config.mistral_api_key.clone(),
            config.mistral_model.clone(),
        );
        if m.ping().await {
            return ProviderKind::Mistral;
        }
    }
    if ping_ollama(config).await {
        return ProviderKind::Ollama;
    }
    ProviderKind::LtOnly
}

/// Ping Ollama (`GET /api/tags`) avec timeout court.
async fn ping_ollama(config: &Config) -> bool {
    let base = config.local.as_ref()
        .map(|l| l.base_url.clone())
        .unwrap_or_else(|| "http://localhost:11434".to_string());
    match probe_client().get(format!("{base}/api/tags")).send().await {
        Ok(r) => r.status().is_success(),
        Err(_) => false,
    }
}

pub fn provider_label(kind: ProviderKind) -> &'static str {
    match kind {
        ProviderKind::Mistral => "Mistral API",
        ProviderKind::Ollama  => "Ollama (local)",
        ProviderKind::LtOnly  => "LanguageTool seul",
    }
}

/// Message d'information affiché lors de la détection initiale du provider.
pub fn provider_startup_message(kind: ProviderKind) -> String {
    match kind {
        ProviderKind::LtOnly => "IA indisponible → LanguageTool seul".to_string(),
        other                => format!("IA active : {}", provider_label(other)),
    }
}

/// Bulle de notification système, sans toucher au tooltip du tray.
pub fn notify(app: &tauri::AppHandle, message: &str) {
    let identifier = app.config().tauri.bundle.identifier.clone();
    let _ = tauri::api::notification::Notification::new(identifier)
        .title("ReformoJuste")
        .body(message)
        .show();
    eprintln!("[notify] {message}");
}

/// Notification d'état du provider : bulle système **et** tooltip du tray, ce
/// dernier restant l'indicateur permanent de l'IA active.
pub fn notify_tray(app: &tauri::AppHandle, message: &str) {
    let _ = app.tray_handle().set_tooltip(&format!("ReformoJuste — {message}"));
    notify(app, message);
}

// ── Factory ───────────────────────────────────────────────────────────────────

pub async fn process(config: &Config, text: &str) -> Result<AiResult> {
    let provider: Box<dyn AiProvider> = match config.ai_provider.as_str() {
        "local" => {
            let c = config.local.clone().ok_or_else(|| anyhow::anyhow!("local config missing"))?;
            Box::new(local::LocalProvider::new(c))
        }
        "languagetool" => {
            let c = config.languagetool.clone().ok_or_else(|| anyhow::anyhow!("languagetool config missing"))?;
            Box::new(languagetool::LanguageToolProvider::new(c))
        }
        "mistral" => {
            Box::new(mistral::MistralProvider::new(
                config.mistral_api_key.clone(),
                config.mistral_model.clone(),
            ))
        }
        "hybrid" => {
            let lt_cfg = config.languagetool.clone().ok_or_else(|| anyhow::anyhow!("languagetool config missing for hybrid mode"))?;
            let ai_cfg = config.local.clone().ok_or_else(|| anyhow::anyhow!("local config missing for hybrid mode"))?;
            let lt = Box::new(languagetool::LanguageToolProvider::new(lt_cfg));
            let ai = Box::new(local::LocalProvider::new(ai_cfg));
            Box::new(hybrid::HybridProvider::new(lt, ai))
        }
        "auto" => {
            match auto_detect_provider(config).await {
                ProviderKind::Mistral => Box::new(mistral::MistralProvider::new(
                    config.mistral_api_key.clone(),
                    config.mistral_model.clone(),
                )),
                ProviderKind::Ollama => {
                    let c = config.local.clone().ok_or_else(|| anyhow::anyhow!("local config missing"))?;
                    Box::new(local::LocalProvider::new(c))
                }
                ProviderKind::LtOnly => {
                    let c = config.languagetool.clone().ok_or_else(|| anyhow::anyhow!("languagetool config missing"))?;
                    Box::new(languagetool::LanguageToolProvider::new(c))
                }
            }
        }
        other => anyhow::bail!("Unknown ai_provider: {}", other),
    };

    provider.process(text).await
}

// ── Streaming processor ───────────────────────────────────────────────────────
//
// In hybrid mode: LanguageTool and Ollama run concurrently.
// LT emits "ai-partial" as soon as it finishes so the correction tab appears
// immediately. Ollama emits "ai-result" when done (with LT correction merged in).
// For all other providers: emits "ai-result" directly.

pub async fn process_streaming(
    config: &Config,
    text:   &str,
    window: &tauri::Window,
    gen:    u64,
) -> Result<()> {
    if config.ai_provider == "auto" {
        return process_auto(config, text, window, gen).await;
    }
    if config.ai_provider == "hybrid" {
        let lt_cfg = config.languagetool.clone()
            .ok_or_else(|| anyhow::anyhow!("languagetool config missing for hybrid mode"))?;
        let ai_cfg = config.local.clone()
            .ok_or_else(|| anyhow::anyhow!("local config missing for hybrid mode"))?;

        let text_lt  = text.to_string();
        let text_ai  = text.to_string();
        let win_lt   = window.clone();

        let lt_slot: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
        let lt_slot_task = lt_slot.clone();

        let lt_handle = tokio::spawn(async move {
            match languagetool::LanguageToolProvider::new(lt_cfg).process(&text_lt).await {
                Ok(r) => {
                    *lt_slot_task.lock().await = Some(r.correction.clone());
                    emit(&win_lt, gen, "ai-partial", PartialResult { correction: r.correction });
                }
                Err(e) => eprintln!("[LT] {e}"),
            }
        });

        let ai_handle = tokio::spawn(async move {
            local::LocalProvider::new(ai_cfg).process(&text_ai).await
        });

        let ai_result = ai_handle.await
            .map_err(|e| anyhow::anyhow!("{e}"))
            .and_then(|r| r);
        let _ = lt_handle.await;

        let lt_correction = lt_slot.lock().await.clone();

        match ai_result {
            Ok(ai_res) => {
                let correction = lt_correction.unwrap_or_else(|| ai_res.correction.clone());
                emit(window, gen, "ai-result", AiResult { correction, ..ai_res });
            }
            Err(_) => {
                // Ollama unavailable: show LT correction only if it succeeded
                if let Some(correction) = lt_correction {
                    emit(window, gen, "ai-result",
                        AiResult::correction_only(correction, "IA locale non disponible (Ollama)"));
                } else {
                    anyhow::bail!("Ollama et LanguageTool sont tous deux indisponibles");
                }
            }
        }
    } else {
        let result = process(config, text).await?;
        emit(window, gen, "ai-result", result);
    }
    Ok(())
}

// ── Mode auto : exécution avec fallback à chaud ────────────────────────────────
//
// Suit le ProviderKind stocké dans l'état Tauri. Si le provider échoue en cours
// d'utilisation (Mistral indispo, quota 429…), on bascule sur le suivant de la
// chaîne, on met à jour l'état partagé et on notifie l'utilisateur via le tray.
// Passé RECOVERY_AFTER, on re-sonde le haut de la chaîne pour remonter tout seul.

/// Provider à utiliser maintenant : détection initiale, état courant, ou re-sondage.
async fn resolve_provider(config: &Config, app: &tauri::AppHandle) -> ProviderKind {
    let state = app.state::<ActiveProvider>();
    let mut s = state.0.lock().await;

    match (s.kind, s.downgraded_at) {
        // Haut de chaîne : rien à retenter.
        (Some(ProviderKind::Mistral), _) => ProviderKind::Mistral,

        // Rétrogradé depuis assez longtemps : on re-sonde.
        (Some(previous), Some(since)) if since.elapsed() >= RECOVERY_AFTER => {
            let fresh = auto_detect_provider(config).await;
            s.kind = Some(fresh);
            s.downgraded_at = (fresh != ProviderKind::Mistral).then(Instant::now);
            if fresh != previous {
                notify_tray(app, &format!("Reprise → {}", provider_label(fresh)));
            }
            fresh
        }

        (Some(current), _) => current,

        // Première utilisation avant la fin de la détection lancée au démarrage.
        (None, _) => {
            let fresh = auto_detect_provider(config).await;
            s.kind = Some(fresh);
            s.downgraded_at = (fresh != ProviderKind::Mistral).then(Instant::now);
            fresh
        }
    }
}

/// Rétrograde l'état partagé et prévient l'utilisateur.
async fn downgrade(app: &tauri::AppHandle, to: ProviderKind, message: &str) {
    {
        let state = app.state::<ActiveProvider>();
        let mut s = state.0.lock().await;
        s.kind = Some(to);
        s.downgraded_at = Some(Instant::now());
    }
    notify_tray(app, message);
}

async fn process_auto(
    config: &Config,
    text:   &str,
    window: &tauri::Window,
    gen:    u64,
) -> Result<()> {
    let app = window.app_handle();

    match resolve_provider(config, &app).await {
        ProviderKind::Mistral => {
            let provider = mistral::MistralProvider::new(
                config.mistral_api_key.clone(),
                config.mistral_model.clone(),
            );
            match provider.process(text).await {
                Ok(result) => {
                    emit(window, gen, "ai-result", result);
                    Ok(())
                }
                Err(e) => {
                    eprintln!("[Mistral] indisponible : {e}");
                    if ping_ollama(config).await {
                        downgrade(&app, ProviderKind::Ollama,
                            "Mistral indisponible → bascule sur Ollama").await;
                        run_ollama(config, text, window, gen).await
                    } else {
                        downgrade(&app, ProviderKind::LtOnly,
                            "Mistral indisponible → bascule sur LanguageTool").await;
                        run_lt_only(config, text, window, gen).await
                    }
                }
            }
        }
        ProviderKind::Ollama => {
            match run_ollama(config, text, window, gen).await {
                Ok(()) => Ok(()),
                Err(e) => {
                    eprintln!("[Ollama] indisponible : {e}");
                    downgrade(&app, ProviderKind::LtOnly,
                        "Ollama indisponible → bascule sur LanguageTool").await;
                    run_lt_only(config, text, window, gen).await
                }
            }
        }
        ProviderKind::LtOnly => run_lt_only(config, text, window, gen).await,
    }
}

/// Ollama local : correction + 5 reformulations via LocalProvider.
async fn run_ollama(
    config: &Config,
    text:   &str,
    window: &tauri::Window,
    gen:    u64,
) -> Result<()> {
    let ai_cfg = config.local.clone()
        .ok_or_else(|| anyhow::anyhow!("config local manquante pour Ollama"))?;
    let result = local::LocalProvider::new(ai_cfg).process(text).await?;
    emit(window, gen, "ai-result", result);
    Ok(())
}

/// LanguageTool seul : correction uniquement, pas de reformulations.
/// La JVM n'est démarrée qu'ici — inutile de payer 875 Mo de RAM tant que
/// Mistral ou Ollama répondent.
async fn run_lt_only(
    config: &Config,
    text:   &str,
    window: &tauri::Window,
    gen:    u64,
) -> Result<()> {
    let lt_cfg = config.languagetool.clone()
        .ok_or_else(|| anyhow::anyhow!("config languagetool manquante"))?;

    let app = window.app_handle();
    if let Some(lt) = app.try_state::<crate::LtProcess>() {
        if lt.ensure_started(config) {
            emit(window, gen, "ai-status", "Démarrage du correcteur local (~10 s)…");
        }
    }

    let res = languagetool::LanguageToolProvider::new(lt_cfg).process(text).await?;

    emit(window, gen, "ai-partial", PartialResult { correction: res.correction.clone() });
    emit(window, gen, "ai-result",
        AiResult::correction_only(res.correction, "Reformulations indisponibles (mode LanguageTool seul)"));
    Ok(())
}

// ── Shared prompt builder ─────────────────────────────────────────────────────

pub fn build_prompt(text: &str) -> String {
    format!(
        r#"Tu es un assistant de correction et reformulation en FRANÇAIS. Tu dois TOUJOURS répondre en français, quelle que soit la langue du texte.

Analyse ce texte et retourne UNIQUEMENT un objet JSON valide avec exactement ces 6 clés (toutes les valeurs doivent être en français) :
{{
  "correction":   "texte corrigé orthographiquement et grammaticalement en français, sans changer le sens",
  "simple":       "reformulation en français, langage simple et accessible",
  "professional": "reformulation en français, style professionnel",
  "formal":       "reformulation en français, style soutenu et élaboré",
  "short":        "réécriture en français, raccourcie au maximum en gardant l'essentiel",
  "creative":     "réécriture en français, créative et originale"
}}

RÈGLE ABSOLUE — forme d'adresse : conserve exactement celle du texte d'origine.
S'il tutoie, les 6 valeurs tutoient. S'il vouvoie, les 6 vouvoient. Ne convertis
JAMAIS « tu » en « vous » ni l'inverse.
Les 5 styles portent sur le vocabulaire et la syntaxe, jamais sur la forme
d'adresse : "professional" et "formal" restent au tutoiement si l'original tutoie
(ex. « Aurais-tu l'obligeance de… ») ; "simple" et "creative" restent au
vouvoiement si l'original vouvoie.

Texte à analyser : {text}

IMPORTANT : Réponds UNIQUEMENT avec le JSON en français, aucun texte avant ou après."#,
        text = text
    )
}

// ── JSON extractor ────────────────────────────────────────────────────────────

pub fn parse_response(raw: &str) -> Result<AiResult> {
    let json = if let (Some(s), Some(e)) = (raw.find('{'), raw.rfind('}')) {
        &raw[s..=e]
    } else {
        raw
    };
    let result: AiResult = serde_json::from_str(json)
        .map_err(|e| anyhow::anyhow!("JSON parse error: {}\nRaw: {}", e, json))?;

    // `correction` est le seul champ non négociable : les modèles locaux
    // renvoient parfois un `{}` vide, qui ne doit pas passer pour un succès.
    if result.correction.trim().is_empty() {
        anyhow::bail!("Réponse du modèle sans correction exploitable : {json}");
    }
    Ok(result)
}
