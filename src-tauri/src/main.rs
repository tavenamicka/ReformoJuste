#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod ai;
mod clipboard;
mod config;
mod cursor;
mod hotkey;

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x08000000;
use tauri::{CustomMenuItem, Manager, RunEvent, SystemTray, SystemTrayEvent, SystemTrayMenu};

use config::Config;

// ── Managed state ─────────────────────────────────────────────────────────────

pub struct AppConfig(pub Arc<Config>);

// ── LanguageTool JAR ──────────────────────────────────────────────────────────

fn find_java(exe_dir: &Path) -> PathBuf {
    // Prefer bundled JRE alongside the exe, fall back to system java
    let bundled = exe_dir.join("bundle").join("jre").join("bin").join("java.exe");
    if bundled.exists() {
        return bundled;
    }
    PathBuf::from("java")
}

fn find_jar(exe_dir: &Path) -> Option<PathBuf> {
    let mut candidates = Vec::new();

    // Production: bundle/ next to exe
    candidates.push(exe_dir.join("bundle").join("languagetool").join("languagetool-server.jar"));

    // Dev: walk up from target/debug/ → target/ → src-tauri/ → project root
    let mut dir = exe_dir.to_path_buf();
    for _ in 0..4 {
        if let Some(p) = dir.parent() {
            dir = p.to_path_buf();
            candidates.push(dir.join("bundle").join("languagetool").join("languagetool-server.jar"));
        }
    }

    candidates.into_iter().find(|p| p.exists())
}

fn spawn_languagetool(config: &Config) -> Option<Child> {
    let exe_dir = std::env::current_exe().ok()?.parent()?.to_path_buf();
    let jar  = find_jar(&exe_dir)?;
    let java = find_java(&exe_dir);

    let port = config.languagetool.as_ref()
        .and_then(|lt| lt.base_url.rsplit(':').next().map(str::to_string))
        .unwrap_or_else(|| "8082".to_string());

    let mut cmd = Command::new(&java);
    cmd.args([
            "-Dfile.encoding=UTF-8",
            "-jar", jar.to_str().unwrap_or(""),
            "--port", &port,
            "--allow-origin", "*",
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .stdin(Stdio::null());

    #[cfg(target_os = "windows")]
    cmd.creation_flags(CREATE_NO_WINDOW);

    match cmd.spawn() {
        Ok(child) => {
            eprintln!("[LT] started with JAR: {}", jar.display());
            Some(child)
        }
        Err(e) => {
            eprintln!("[LT] cannot start JAR ({}): {e}", jar.display());
            None
        }
    }
}

/// Processus LanguageTool, démarré **à la demande**.
///
/// La JVM + les règles françaises pèsent ~875 Mo de RAM et ~8 s de démarrage.
/// En mode `auto`, LanguageTool n'est que le dernier maillon de la chaîne de
/// repli : le lancer systématiquement au démarrage faisait payer ce coût à
/// chaque session, y compris quand Mistral répondait en 2 s.
pub struct LtProcess(pub Arc<Mutex<Option<Child>>>);

impl LtProcess {
    fn new() -> Self {
        Self(Arc::new(Mutex::new(None)))
    }

    /// Démarre la JVM si elle ne tourne pas (ou plus).
    /// Retourne `true` si un démarrage vient d'être déclenché — l'appelant sait
    /// alors qu'il faut prévenir l'utilisateur de l'attente.
    pub fn ensure_started(&self, config: &Config) -> bool {
        let mut slot = match self.0.lock() {
            Ok(s)  => s,
            Err(p) => p.into_inner(),
        };

        if let Some(child) = slot.as_mut() {
            match child.try_wait() {
                Ok(None) => return false,   // toujours vivante
                _        => *slot = None,   // terminée ou illisible : on relance
            }
        }

        match spawn_languagetool(config) {
            Some(child) => { *slot = Some(child); true }
            None        => false,
        }
    }

    fn stop(&self) {
        if let Ok(mut slot) = self.0.lock() {
            if let Some(mut child) = slot.take() {
                let _ = child.kill();
            }
        }
    }
}

// ── Tauri commands ────────────────────────────────────────────────────────────

#[tauri::command]
async fn process_text(
    text:   String,
    window: tauri::Window,
    state:  tauri::State<'_, AppConfig>,
) -> Result<(), String> {
    let config = state.0.clone();
    // Chaque capture invalide la précédente : un résultat lent qui arrive après
    // un nouveau déclenchement sera ignoré au lieu d'écraser l'affichage.
    let gen = ai::next_generation();
    tokio::spawn(async move {
        if let Err(e) = ai::process_streaming(&config, &text, &window, gen).await {
            ai::emit(&window, gen, "ai-error", e.to_string());
        }
    });
    Ok(())
}

#[tauri::command]
fn copy_to_clipboard(text: String) -> Result<(), String> {
    clipboard::write(&text).map_err(|e| e.to_string())
}

#[tauri::command]
async fn replace_text(text: String, window: tauri::Window) -> Result<(), String> {
    clipboard::write(&text).map_err(|e| e.to_string())?;
    let _ = window.hide();
    tokio::task::spawn_blocking(|| {
        // Laisse le focus revenir à l'application d'origine avant le Ctrl+V.
        std::thread::sleep(std::time::Duration::from_millis(120));
        clipboard::simulate_paste()
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| e.to_string())
}

// ── Entry point ───────────────────────────────────────────────────────────────

fn main() {
    let config = Arc::new(
        config::load().expect("Cannot load config.json — place it next to the executable"),
    );

    let lt = LtProcess::new();
    // Les modes qui s'appuient sur LanguageTool à chaque requête le démarrent
    // tout de suite ; en mode `auto` il reste dormant jusqu'au repli effectif.
    if config.ai_provider == "hybrid" || config.ai_provider == "languagetool" {
        lt.ensure_started(&config);
    }

    // Provider actif en mode auto : `None` tant que la détection asynchrone
    // lancée dans `.setup` n'a pas abouti (un déclenchement précoce la force).
    let active_provider: Arc<tokio::sync::Mutex<ai::AutoState>> =
        Arc::new(tokio::sync::Mutex::new(ai::AutoState::default()));

    let quit = CustomMenuItem::new("quit", "Quitter ReformoJuste");
    let tray = SystemTray::new()
        .with_menu(SystemTrayMenu::new().add_item(quit))
        .with_tooltip("ReformoJuste — Double Ctrl+Space");

    let app = tauri::Builder::default()
        .system_tray(tray)
        .on_system_tray_event(|app, event| {
            if let SystemTrayEvent::MenuItemClick { id, .. } = event {
                if id == "quit" {
                    app.state::<LtProcess>().stop();
                    app.exit(0);
                }
            }
        })
        .manage(AppConfig(config.clone()))
        .manage(ai::ActiveProvider(active_provider.clone()))
        .manage(lt)
        .setup(move |app| {
            hotkey::start(app.handle(), config.clone());

            // Détection automatique du provider (Mistral → Ollama → LT seul).
            if config.ai_provider == "auto" {
                let cfg = config.clone();
                let ap = active_provider.clone();
                let handle = app.handle();
                tauri::async_runtime::spawn(async move {
                    let kind = ai::auto_detect_provider(&cfg).await;
                    {
                        let mut s = ap.lock().await;
                        s.kind = Some(kind);
                        s.downgraded_at =
                            (kind != ai::ProviderKind::Mistral).then(std::time::Instant::now);
                    }
                    eprintln!("[AI] provider auto-détecté : {kind:?}");
                    ai::notify_tray(&handle, &ai::provider_startup_message(kind));

                    // Ollama décharge son modèle après 5 min d'inactivité : on
                    // le précharge pour que la première correction ne paie pas
                    // les ~20 s de chargement.
                    if kind == ai::ProviderKind::Ollama {
                        if let Some(local) = cfg.local.clone() {
                            ai::local::LocalProvider::new(local).warm_up().await;
                        }
                    }
                });
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![process_text, copy_to_clipboard, replace_text])
        .build(tauri::generate_context!())
        .expect("error while building ReformoJuste");

    // Ne jamais laisser la JVM LanguageTool survivre à l'application.
    app.run(|app, event| {
        if let RunEvent::Exit = event {
            app.state::<LtProcess>().stop();
        }
    });
}
