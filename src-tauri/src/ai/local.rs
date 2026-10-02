use anyhow::Result;
use async_trait::async_trait;
use reqwest::Client;
use serde_json::json;

use crate::config::LocalConfig;
use super::{two_pass, AiProvider, AiResult, Completion, LlmProvider};

/// Durée pendant laquelle Ollama garde le modèle chargé en mémoire.
/// Par défaut Ollama le décharge au bout de 5 min : l'usage sporadique de
/// ReformoJuste retombait donc quasi systématiquement sur un chargement à
/// froid (~20 s mesurés sur gemma3:4b) pour ~1 s d'inférence réelle.
const KEEP_ALIVE: &str = "30m";

// Le plafond de génération n'est plus une constante de ce fichier : il arrive
// par `Completion::max_tokens`, chaque passe ayant son budget (cf. ai/mod.rs).
// Borne un modèle qui part en boucle plutôt que de laisser la popup tourner
// jusqu'au timeout.
//
// Limite connue : sur CPU, générer les 3072 tokens de la passe de reformulation
// dépasse le timeout de 180 s bien avant le plafond. Ollama reste donc le repli
// de dernier recours, utilisable sur des textes courts.

pub struct LocalProvider {
    config: LocalConfig,
    client: Client,
}

impl LocalProvider {
    pub fn new(config: LocalConfig) -> Self {
        // Client partagé (pool de connexions + timeout) — cf. ai/mod.rs.
        Self { config, client: super::local_client() }
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
    async fn call_ollama(&self, req: &Completion<'_>) -> Result<String> {
        let body = json!({
            "model":      self.config.model,
            "system":     req.system,
            "prompt":     req.user,
            "stream":     false,
            "format":     "json",
            "keep_alive": KEEP_ALIVE,
            "options":    { "temperature": req.temperature, "num_predict": req.max_tokens }
        });

        let resp = self.client
            .post(format!("{}/api/generate", self.config.base_url))
            .json(&body)
            .send()
            .await?;

        let data: serde_json::Value = resp.json().await?;
        data["response"]
            .as_str()
            .map(str::to_string)
            .ok_or_else(|| anyhow::anyhow!("Empty Ollama response: {:?}", data))
    }

    /// LM Studio / any OpenAI-compatible local server
    async fn call_openai_compat(&self, req: &Completion<'_>) -> Result<String> {
        let body = json!({
            "model": self.config.model,
            "messages": [
                { "role": "system", "content": req.system },
                { "role": "user",   "content": req.user }
            ],
            "temperature": req.temperature,
            "max_tokens": req.max_tokens
        });

        let resp = self.client
            .post(format!("{}/v1/chat/completions", self.config.base_url))
            .json(&body)
            .send()
            .await?;

        let data: serde_json::Value = resp.json().await?;
        data["choices"][0]["message"]["content"]
            .as_str()
            .map(str::to_string)
            .ok_or_else(|| anyhow::anyhow!("Empty local response: {:?}", data))
    }
}

#[async_trait]
impl LlmProvider for LocalProvider {
    async fn complete(&self, req: Completion<'_>) -> Result<String> {
        match self.config.provider.as_str() {
            "ollama"              => self.call_ollama(&req).await,
            "lmstudio"
            | "openai_compatible" => self.call_openai_compat(&req).await,
            other => anyhow::bail!("Unknown local provider: {}", other),
        }
    }

    /// Les deux passes sont sérialisées en local : sur un portable sans GPU,
    /// deux générations concurrentes se disputent les mêmes cœurs et doublent
    /// la mémoire de contexte — au total plus lent qu'en série.
    fn parallel_passes(&self) -> bool {
        false
    }
}

#[async_trait]
impl AiProvider for LocalProvider {
    async fn process(&self, text: &str) -> Result<AiResult> {
        two_pass(self, text).await
    }
}
