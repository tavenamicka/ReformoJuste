use anyhow::Result;
use async_trait::async_trait;
use reqwest::Client;
use serde_json::json;

use crate::config::LocalConfig;
use super::{AiProvider, AiResult, build_prompt, parse_response};

/// Durée pendant laquelle Ollama garde le modèle chargé en mémoire.
/// Par défaut Ollama le décharge au bout de 5 min : l'usage sporadique de
/// ReformoJuste retombait donc quasi systématiquement sur un chargement à
/// froid (~20 s mesurés sur gemma3:4b) pour ~1 s d'inférence réelle.
const KEEP_ALIVE: &str = "30m";

/// Plafond de génération : 6 champs ≈ 200 tokens. Borne un modèle qui part
/// en boucle plutôt que de laisser la popup tourner jusqu'au timeout.
const MAX_TOKENS: u32 = 512;

pub struct LocalProvider {
    config: LocalConfig,
    client: Client,
}

impl LocalProvider {
    pub fn new(config: LocalConfig) -> Self {
        // Client partagé (pool de connexions + timeout) — cf. ai/mod.rs.
        Self { config, client: super::ai_client() }
    }

    /// Précharge le modèle en mémoire sans rien générer (prompt vide).
    /// Appelé au démarrage quand Ollama est le provider retenu, pour que la
    /// première correction ne paie pas le chargement du modèle.
    pub async fn warm_up(&self) {
        if self.config.provider != "ollama" {
            return;
        }
        let body = json!({
            "model":      self.config.model,
            "prompt":     "",
            "stream":     false,
            "keep_alive": KEEP_ALIVE,
        });
        match self.client
            .post(format!("{}/api/generate", self.config.base_url))
            .json(&body)
            .send()
            .await
        {
            Ok(_)  => eprintln!("[Ollama] modèle {} préchargé", self.config.model),
            Err(e) => eprintln!("[Ollama] préchargement échoué : {e}"),
        }
    }

    /// Ollama: POST /api/generate with format:"json"
    async fn call_ollama(&self, text: &str) -> Result<AiResult> {
        let body = json!({
            "model":      self.config.model,
            "prompt":     build_prompt(text),
            "stream":     false,
            "format":     "json",
            "keep_alive": KEEP_ALIVE,
            "options":    { "num_predict": MAX_TOKENS }
        });

        let resp = self.client
            .post(format!("{}/api/generate", self.config.base_url))
            .json(&body)
            .send()
            .await?;

        let data: serde_json::Value = resp.json().await?;
        let content = data["response"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Empty Ollama response: {:?}", data))?;

        parse_response(content)
    }

    /// LM Studio / any OpenAI-compatible local server
    async fn call_openai_compat(&self, text: &str) -> Result<AiResult> {
        let body = json!({
            "model": self.config.model,
            "messages": [{"role": "user", "content": build_prompt(text)}],
            "temperature": 0.7,
            "max_tokens": MAX_TOKENS
        });

        let resp = self.client
            .post(format!("{}/v1/chat/completions", self.config.base_url))
            .json(&body)
            .send()
            .await?;

        let data: serde_json::Value = resp.json().await?;
        let content = data["choices"][0]["message"]["content"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Empty local response: {:?}", data))?;

        parse_response(content)
    }
}

#[async_trait]
impl AiProvider for LocalProvider {
    async fn process(&self, text: &str) -> Result<AiResult> {
        match self.config.provider.as_str() {
            "ollama"           => self.call_ollama(text).await,
            "lmstudio"
            | "openai_compatible" => self.call_openai_compat(text).await,
            other => anyhow::bail!("Unknown local provider: {}", other),
        }
    }
}
