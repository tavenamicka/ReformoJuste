use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Config {
    pub ai_provider:  String,
    /// Clé API Mistral. Vide = Mistral désactivé (détection auto le saute).
    #[serde(default)]
    pub mistral_api_key: String,
    #[serde(default = "default_mistral_model")]
    pub mistral_model:   String,
    /// Clé API Google Gemini. Vide = Gemini désactivé (détection auto le saute).
    #[serde(default)]
    pub gemini_api_key: String,
    #[serde(default = "default_gemini_model")]
    pub gemini_model:   String,
    pub local:        Option<LocalConfig>,
    pub languagetool: Option<LanguageToolConfig>,
    pub hotkey:       HotkeyConfig,
    pub popup:        PopupConfig,
}

fn default_mistral_model() -> String {
    "mistral-small-latest".to_string()
}

fn default_gemini_model() -> String {
    // `gemini-2.5-flash` renvoie 404 « no longer available to new users »
    // (mesuré 2026-10-02) : le maillon Gemini de la chaîne de repli était mort
    // pour toute nouvelle clé. `/v1beta/openai/models` le liste pourtant
    // encore — le catalogue n'est pas un droit d'accès, et `ping()` répondait
    // donc 200 pour un modèle inutilisable.
    "gemini-3.6-flash".to_string()
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LocalConfig {
    pub base_url: String,
    pub model:    String,
    /// "ollama" | "lmstudio" | "openai_compatible"
    pub provider: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LanguageToolConfig {
    pub base_url: String,
    #[serde(default = "default_lt_language")]
    pub language: String,
}

fn default_lt_language() -> String {
    "fr".to_string()
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct HotkeyConfig {
    pub double_press_ms: u64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PopupConfig {
    pub width:    u32,
    pub height:   u32,
    pub offset_x: i32,
    pub offset_y: i32,
}

fn config_path() -> PathBuf {
    // 1. Beside the executable (production)
    if let Ok(exe) = std::env::current_exe() {
        let exe_dir = exe.parent().unwrap();
        let p = exe_dir.join("config.json");
        if p.exists() {
            return p;
        }
        // 2. Walk up from exe dir (dev: target/debug/ → project root)
        let mut dir = exe_dir;
        for _ in 0..4 {
            if let Some(parent) = dir.parent() {
                let p = parent.join("config.json");
                if p.exists() {
                    return p;
                }
                dir = parent;
            }
        }
    }
    // 3. Current working directory
    std::env::current_dir()
        .unwrap_or_default()
        .join("config.json")
}

/// Charge une config depuis un chemin donné. Séparé de [`load`] pour que les
/// bancs de mesure puissent pointer un autre `config.json` (cf. `ai/mod.rs`,
/// module `mesure`) sans toucher à celui de développement.
pub fn load_from(path: &Path) -> Result<Config> {
    let content = fs::read_to_string(path)
        .map_err(|e| anyhow::anyhow!("Cannot read {}: {}", path.display(), e))?;
    // Le Bloc-notes / PowerShell 5.1 ajoutent un BOM UTF-8 que serde_json refuse.
    let cfg: Config = serde_json::from_str(content.trim_start_matches('\u{feff}'))?;
    Ok(cfg)
}

pub fn load() -> Result<Config> {
    load_from(&config_path())
}
