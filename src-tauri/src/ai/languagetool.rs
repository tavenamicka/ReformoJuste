use anyhow::Result;
use async_trait::async_trait;
use reqwest::Client;
use serde::Deserialize;
use tokio::time::{sleep, Duration};

use crate::config::LanguageToolConfig;
use super::{AiProvider, AiResult};

// La JVM LanguageTool met ~8 s à répondre après un démarrage à froid. On sonde
// plus souvent que l'ancien 8×1500 ms : même budget total, mais on repart dès
// que le serveur est prêt au lieu d'attendre le prochain palier de 1,5 s.
const MAX_RETRIES: u32 = 14;
const RETRY_DELAY_MS: u64 = 900;

// ── LanguageTool API response types ──────────────────────────────────────────

#[derive(Deserialize)]
struct LtResponse {
    #[serde(default)]
    matches: Vec<LtMatch>,
}

#[derive(Deserialize)]
struct LtMatch {
    /// Index en **unités UTF-16** (cf. `apply_corrections`), pas en caractères.
    offset: usize,
    length: usize,
    #[serde(default)]
    replacements: Vec<LtReplacement>,
    #[serde(default)]
    rule: LtRule,
}

#[derive(Deserialize)]
struct LtReplacement {
    #[serde(default)]
    value: String,
}

#[derive(Default, Deserialize)]
#[serde(default)]
struct LtRule {
    id: String,
    #[serde(rename = "issueType")]
    issue_type: String,
}

// ── Sélection des suggestions ─────────────────────────────────────────────────
//
// L'ancienne version appliquait `replacements[0]` de **toutes** les règles. Or
// LanguageTool ne classe pas ses suggestions par confiance : pour un mot qu'il
// ne connaît pas, il énumère les voisins orthographiques et la première entrée
// est souvent un autre mot. D'où des corrections qui remplaçaient un mot par un
// mot sans rapport.
//
// Mesuré sur le serveur bundlé (`--port 8099`, français), 25 cas :
//
// | segment      | règle                   | nrep | 1ʳᵉ suggestion | juste ? |
// |--------------|-------------------------|------|----------------|---------|
// | `recu`       | FR_SPELLING_RULE        |   25 | `reçu`         | oui     |
// | `tres`       | FR_SPELLING_RULE        |   41 | `très`         | oui     |
// | `ortographe` | FR_SPELLING_RULE        |    4 | `orthographe`  | oui     |
// | `develloper` | FR_SPELLING_RULE        |    9 | `développer`   | oui     |
// | `mesage`     | FR_SPELLING_RULE        |   14 | `mes age`      | NON     |
// | `Zyglub`     | FR_SPELLING_RULE        |   25 | `Club`         | NON     |
// | `conpuseur`  | FR_SPELLING_RULE        |    7 | `confesseur`   | NON     |
// | `Les chien`  | D_N                     |    4 | `Le chien`     | NON     |
// | `tu fait`    | ACCORD_R_PERS_VERBE     |    1 | `fais`         | oui     |
//
// Le nombre de suggestions ne discrimine donc rien — filtrer sur « une seule
// suggestion » aurait jeté `reçu` et `très`, la classe de faute française la
// plus courante. Deux critères séparent en revanche proprement les colonnes :
// la suggestion reste **un seul mot** (elle ne coupe pas le mot en deux), et
// elle reste **proche** du segment d'origine.
//
// Filtrer sur `issueType` seul ne marchait pas non plus : la plupart des règles
// de grammaire françaises (ACCORD_R_PERS_VERBE, IMP_PRON, OU, LEURS_LEUR,
// A_A_ACCENT) sont marquées `uncategorized`. Seules les règles de préférence
// sont écartées par ce biais.

/// `issueType` des règles qui expriment une préférence, pas une faute.
const IGNORED_ISSUE_TYPES: &[&str] = &["style", "register"];

/// Règles classées `style` par LanguageTool alors qu'elles corrigent une vraie
/// faute de frappe : un mot dupliqué n'est pas une préférence de style.
/// Mesuré : « Je je viens demain. » → « Je viens demain. », suggestion unique.
/// Sans cette exception, écarter `style` faisait perdre une correction que
/// l'ancienne version réussissait.
const STYLE_EXCEPTIONS: &[&str] = &["FRENCH_WORD_REPEAT_RULE"];

/// Distance d'édition tolérée entre le segment et la suggestion quand la règle
/// en propose plusieurs. Plus stricte sur les mots courts, où deux caractères
/// changés font déjà un autre mot.
fn max_edit_distance(len: usize) -> usize {
    if len <= 5 { 1 } else { 2 }
}

/// Minuscules + diacritiques repliés, pour comparer `recu` à `reçu`.
fn fold(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars().flat_map(char::to_lowercase) {
        out.push_str(match c {
            'à' | 'á' | 'â' | 'ä' | 'ã' => "a",
            'é' | 'è' | 'ê' | 'ë'       => "e",
            'î' | 'ï' | 'í' | 'ì'       => "i",
            'ô' | 'ö' | 'ó' | 'ò' | 'õ' => "o",
            'ù' | 'û' | 'ü' | 'ú'       => "u",
            'ç'                         => "c",
            'ÿ' | 'ý'                   => "y",
            'ñ'                         => "n",
            'œ'                         => "oe",
            'æ'                         => "ae",
            other => {
                out.push(other);
                continue;
            }
        });
    }
    out
}

fn levenshtein(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    let mut cur = vec![0usize; b.len() + 1];

    for (i, ca) in a.iter().enumerate() {
        cur[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let cost = usize::from(ca != cb);
            cur[j + 1] = (prev[j] + cost).min(prev[j + 1] + 1).min(cur[j] + 1);
        }
        std::mem::swap(&mut prev, &mut cur);
    }
    prev[b.len()]
}

fn is_single_word(s: &str) -> bool {
    !s.is_empty() && !s.chars().any(char::is_whitespace)
}

/// Suggestion à appliquer, ou `None` quand la règle n'est pas assez sûre pour
/// toucher au texte — on préfère laisser une faute qu'inventer un mot.
fn accepted_replacement<'a>(m: &'a LtMatch, segment: &str) -> Option<&'a str> {
    if IGNORED_ISSUE_TYPES.contains(&m.rule.issue_type.as_str())
        && !STYLE_EXCEPTIONS.contains(&m.rule.id.as_str())
    {
        return None;
    }

    let first = m.replacements.first()?.value.as_str();

    // Une règle qui ne propose qu'une chose ne laisse pas de place au doute :
    // sur les 25 cas sondés, toutes les règles à suggestion unique étaient
    // justes (accords, traits d'union, homophones, typographie).
    if m.replacements.len() == 1 {
        return Some(first);
    }

    // Plusieurs suggestions : la première n'est qu'un voisin orthographique.
    // On ne l'accepte que si elle ne change pas le découpage en mots…
    if !is_single_word(segment) || !is_single_word(first) {
        return None;
    }

    // …et qu'elle reste le même mot, à quelques signes près.
    let (folded_segment, folded_first) = (fold(segment), fold(first));
    if folded_segment == folded_first {
        return Some(first);
    }
    let distance = levenshtein(&folded_segment, &folded_first);
    (distance <= max_edit_distance(folded_segment.chars().count())).then_some(first)
}

// ── Correction applier ────────────────────────────────────────────────────────

/// `true` si `index` tombe au milieu d'une paire de substituts UTF-16.
fn splits_surrogate(units: &[u16], index: usize) -> bool {
    matches!(units.get(index), Some(u) if (0xDC00..=0xDFFF).contains(u))
}

/// Applique les suggestions retenues.
///
/// Le serveur LanguageTool est écrit en Java : ses `offset`/`length` sont des
/// index de `String` Java, donc des **unités UTF-16**, pas des code points.
/// L'ancienne version travaillait sur un `Vec<char>` en croyant le contraire —
/// identique pour tout le latin accentué, mais décalé dès qu'un caractère hors
/// BMP précède la faute. Mesuré : sur « 😀 Je vais a la maison. », LanguageTool
/// renvoie offset 11 pour un `a` qui est au caractère 10, et l'ancien code
/// remplaçait donc l'espace d'avant, produisant « Je vaisà la maison ».
fn apply_corrections(text: &str, matches: &[LtMatch]) -> String {
    let mut units: Vec<u16> = text.encode_utf16().collect();

    // De la fin vers le début, pour que les offsets restants restent valides.
    let mut ordered: Vec<&LtMatch> = matches.iter().collect();
    ordered.sort_by(|a, b| b.offset.cmp(&a.offset));

    // Début du dernier remplacement appliqué : toute correction qui le dépasse
    // empiète sur une zone déjà réécrite. LanguageTool renvoie des matches qui
    // se recouvrent (règle de mot + règle de groupe), et les appliquer tous
    // mélangeait les deux réécritures.
    let mut boundary = units.len();

    for m in ordered {
        let start = m.offset;
        let Some(end) = m.offset.checked_add(m.length) else { continue };

        // Borne haute **et** borne basse : `start` n'était pas vérifié, et un
        // offset hors bornes faisait paniquer `splice` — tâche tokio morte,
        // popup bloquée sur le chargement.
        if end > units.len() || end > boundary {
            eprintln!("[LT] {} ignoré : {start}..{end} hors bornes ou chevauchant", m.rule.id);
            continue;
        }
        if splits_surrogate(&units, start) || splits_surrogate(&units, end) {
            eprintln!("[LT] {} ignoré : coupe une paire de substituts", m.rule.id);
            continue;
        }

        let segment = String::from_utf16_lossy(&units[start..end]);
        let Some(replacement) = accepted_replacement(m, &segment) else {
            eprintln!("[LT] {} écarté sur « {segment} » (suggestion peu sûre)", m.rule.id);
            continue;
        };

        units.splice(start..end, replacement.encode_utf16().collect::<Vec<u16>>());
        boundary = start;
    }

    // Les suggestions viennent de `&str` valides, donc les paires de substituts
    // restent appariées ; le repli protège d'une réponse serveur incohérente.
    String::from_utf16(&units).unwrap_or_else(|_| text.to_string())
}

// ── Provider ──────────────────────────────────────────────────────────────────

pub struct LanguageToolProvider {
    config: LanguageToolConfig,
    client: Client,
}

impl LanguageToolProvider {
    pub fn new(config: LanguageToolConfig) -> Self {
        // Client partagé (pool de connexions + timeout) — cf. ai/mod.rs.
        Self { config, client: super::lt_client() }
    }

    fn is_connection_error(e: &anyhow::Error) -> bool {
        e.downcast_ref::<reqwest::Error>()
            .map(|re| re.is_connect())
            .unwrap_or(false)
    }

    async fn call(&self, text: &str) -> Result<String> {
        let resp = self.client
            .post(format!("{}/v2/check", self.config.base_url))
            .form(&[("text", text), ("language", &self.config.language)])
            .send()
            .await?;

        let data: LtResponse = resp.json().await?;
        // Toujours renvoyer le texte (corrigé ou non) : la popup diffe la
        // correction contre l'original et affiche « ✓ Aucune faute détectée »
        // quand ils sont identiques. Renvoyer une phrase de statut à la place
        // faisait diffuser tout le texte comme supprimé.
        Ok(apply_corrections(text, &data.matches))
    }
}

#[async_trait]
impl AiProvider for LanguageToolProvider {
    async fn process(&self, text: &str) -> Result<AiResult> {
        let mut last_err = anyhow::anyhow!("no attempt");

        for attempt in 0..MAX_RETRIES {
            match self.call(text).await {
                Ok(correction) => {
                    return Ok(AiResult {
                        correction,
                        ..AiResult::default()
                    });
                }
                Err(e) if Self::is_connection_error(&e) => {
                    eprintln!("[LT] not ready yet (attempt {}/{MAX_RETRIES}), retrying…", attempt + 1);
                    last_err = e;
                    sleep(Duration::from_millis(RETRY_DELAY_MS)).await;
                }
                Err(e) => return Err(e),
            }
        }

        Err(last_err)
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────
//
// Les offsets, règles et listes de suggestions ci-dessous sont **relevés** sur
// le serveur bundlé, pas inventés : c'est ce qui donne leur valeur aux tests.

#[cfg(test)]
mod tests {
    use super::*;

    fn lt_match(offset: usize, length: usize, rule: &str, issue: &str, reps: &[&str]) -> LtMatch {
        LtMatch {
            offset,
            length,
            replacements: reps.iter().map(|v| LtReplacement { value: v.to_string() }).collect(),
            rule: LtRule { id: rule.to_string(), issue_type: issue.to_string() },
        }
    }

    /// Offsets UTF-16 : LanguageTool annonce 11 pour un `a` au caractère 10.
    #[test]
    fn offsets_utf16_avec_emoji() {
        let text = "😀 Je vais a la maison.";
        let matches = [lt_match(11, 1, "A_A_ACCENT", "uncategorized", &["à"])];
        assert_eq!(apply_corrections(text, &matches), "😀 Je vais à la maison.");
    }

    /// Deux emojis = deux unités de décalage chacun.
    #[test]
    fn offsets_utf16_avec_deux_emojis() {
        let text = "😀😁 tu fait des efforts.";
        let matches = [lt_match(8, 4, "ACCORD_R_PERS_VERBE", "uncategorized", &["fais"])];
        assert_eq!(apply_corrections(text, &matches), "😀😁 tu fais des efforts.");
    }

    #[test]
    fn suggestion_unique_appliquee() {
        let text = "tu fait des efforts.";
        let matches = [lt_match(3, 4, "ACCORD_R_PERS_VERBE", "uncategorized", &["fais"])];
        assert_eq!(apply_corrections(text, &matches), "tu fais des efforts.");
    }

    /// 25 et 41 suggestions, mais la première n'est que le mot accentué.
    #[test]
    fn accent_accepte_malgre_de_nombreuses_suggestions() {
        let text = "J'ai bien recu ton message.";
        let matches = [lt_match(10, 4, "FR_SPELLING_RULE", "misspelling",
            &["reçu", "reçut", "vécu", "recul", "déçu"])];
        assert_eq!(apply_corrections(text, &matches), "J'ai bien reçu ton message.");

        let text = "C'est tres bien.";
        let matches = [lt_match(6, 4, "FR_SPELLING_RULE", "misspelling",
            &["très", "près", "tués", "êtres"])];
        assert_eq!(apply_corrections(text, &matches), "C'est très bien.");
    }

    #[test]
    fn lettre_manquante_acceptee() {
        let text = "Mon ortographe est mauvaise.";
        let matches = [lt_match(4, 10, "FR_SPELLING_RULE", "misspelling",
            &["orthographe", "orthographié", "orthographe."])];
        assert_eq!(apply_corrections(text, &matches), "Mon orthographe est mauvaise.");
    }

    /// Le cas « il invente des mots » : suggestion trop éloignée.
    #[test]
    fn mot_sans_rapport_refuse() {
        let text = "J'ai vu Zyglub hier soir.";
        let matches = [lt_match(8, 6, "FR_SPELLING_RULE", "misspelling",
            &["Club", "Zyglu", "Glub"])];
        assert_eq!(apply_corrections(text, &matches), text);

        let text = "Le conpuseur est cassé.";
        let matches = [lt_match(3, 9, "FR_SPELLING_RULE", "misspelling",
            &["confesseur", "compteur"])];
        assert_eq!(apply_corrections(text, &matches), text);
    }

    /// `mesage` → `mes age` : couper un mot en deux n'est pas une correction.
    #[test]
    fn decoupage_de_mot_refuse() {
        let text = "J'ai recu ton mesage.";
        let matches = [lt_match(14, 6, "FR_SPELLING_RULE", "misspelling",
            &["mes age", "m'étage", "message"])];
        assert_eq!(apply_corrections(text, &matches), text);
    }

    /// `Les chien` → `Le chien` alors que la bonne réponse est `Les chiens` :
    /// accord ambigu sur plusieurs mots, on ne tranche pas.
    #[test]
    fn accord_ambigu_multi_mots_refuse() {
        let text = "Les chien sont dans le jardin.";
        let matches = [lt_match(0, 9, "D_N", "uncategorized",
            &["Le chien", "Les chiens", "La chienne", "Les chiennes"])];
        assert_eq!(apply_corrections(text, &matches), text);
    }

    #[test]
    fn regles_de_style_ignorees() {
        let text = "Il a fait une chose bien.";
        let matches = [lt_match(8, 4, "PREFERENCE_STYLISTIQUE", "style", &["réalisé"])];
        assert_eq!(apply_corrections(text, &matches), text);
    }

    /// Un mot dupliqué est une faute de frappe, pas une préférence : exception
    /// explicite à l'exclusion de `style`.
    #[test]
    fn mot_duplique_corrige_malgre_issuetype_style() {
        let text = "Je je viens demain.";
        let matches = [lt_match(0, 5, "FRENCH_WORD_REPEAT_RULE", "style", &["Je"])];
        assert_eq!(apply_corrections(text, &matches), "Je viens demain.");
    }

    /// Un offset hors bornes faisait paniquer `splice`.
    #[test]
    fn offsets_hors_bornes_sans_panique() {
        let text = "Texte court.";
        let matches = [
            lt_match(500, 3, "X", "uncategorized", &["y"]),
            lt_match(10, 50, "Y", "uncategorized", &["z"]),
            lt_match(usize::MAX, 2, "Z", "uncategorized", &["w"]),
        ];
        assert_eq!(apply_corrections(text, &matches), text);
    }

    /// Deux règles sur une zone commune : la seconde est écartée au lieu
    /// d'écraser la réécriture de la première.
    #[test]
    fn matches_chevauchants_une_seule_application() {
        let text = "tu fait des efforts.";
        let matches = [
            lt_match(3, 4, "ACCORD_R_PERS_VERBE", "uncategorized", &["fais"]),
            lt_match(0, 7, "GROUPE", "uncategorized", &["vous faites"]),
        ];
        // L'ordre décroissant applique d'abord 3..7, puis 0..7 chevauche.
        assert_eq!(apply_corrections(text, &matches), "tu fais des efforts.");
    }

    #[test]
    fn plusieurs_corrections_independantes() {
        let text = "tu fait des efforts a la maison.";
        let matches = [
            lt_match(3, 4, "ACCORD_R_PERS_VERBE", "uncategorized", &["fais"]),
            lt_match(20, 1, "A_A_ACCENT", "uncategorized", &["à"]),
        ];
        assert_eq!(apply_corrections(text, &matches), "tu fais des efforts à la maison.");
    }

    #[test]
    fn texte_sans_faute_inchange() {
        let text = "Les chiens sont dans le jardin.";
        assert_eq!(apply_corrections(text, &[]), text);
    }

    #[test]
    fn repli_des_diacritiques() {
        assert_eq!(fold("Reçu"), "recu");
        assert_eq!(fold("TRÈS"), "tres");
        assert_eq!(fold("Idée"), "idee");
        assert_eq!(fold("cœur"), "coeur");
    }

    #[test]
    fn distance_edition() {
        assert_eq!(levenshtein("recu", "recu"), 0);
        assert_eq!(levenshtein("ortographe", "orthographe"), 1);
        assert_eq!(levenshtein("develloper", "developper"), 2);
        assert_eq!(levenshtein("zyglub", "club"), 3);
    }
}
