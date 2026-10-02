use anyhow::Result;
use async_trait::async_trait;
use reqwest::Client;
use serde_json::json;

use crate::config::LocalConfig;
use super::{two_pass, AiProvider, AiResult, LlmProvider};

/// Durée pendant laquelle Ollama garde le modèle chargé en mémoire.
/// Par défaut Ollama le décharge au bout de 5 min : l'usage sporadique de
/// ReformoJuste retombait donc quasi systématiquement sur un chargement à
/// froid (~20 s mesurés sur gemma3:4b) pour ~1 s d'inférence réelle.
const KEEP_ALIVE: &str = "30m";

/// Plafond de génération par appel. Borne un modèle qui part en boucle plutôt
/// que de laisser la popup tourner jusqu'au timeout.
///
/// Limite connue : confortable pour la passe de correction (un seul champ),
/// juste pour la passe de reformulation (5 champs) au-delà de quelques phrases
/// — `format: "json"` fait alors fermer le JSON prématurément, donc des champs
/// tronqués sans erreur remontée.
const MAX_TOKENS: u32 = 512;

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
    async fn call_ollama(&self, system: &str, user: &str, temperature: f32) -> Result<String> {
        let body = json!({
            "model":      self.config.model,
            "system":     system,
            "prompt":     user,
            "stream":     false,
            "format":     "json",
            "keep_alive": KEEP_ALIVE,
            "options":    { "temperature": temperature, "num_predict": MAX_TOKENS }
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
    async fn call_openai_compat(&self, system: &str, user: &str, temperature: f32) -> Result<String> {
        let body = json!({
            "model": self.config.model,
            "messages": [
                { "role": "system", "content": system },
                { "role": "user",   "content": user }
            ],
            "temperature": temperature,
            "max_tokens": MAX_TOKENS
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
    async fn complete(&self, system: &str, user: &str, temperature: f32) -> Result<String> {
        match self.config.provider.as_str() {
            "ollama"              => self.call_ollama(system, user, temperature).await,
            "lmstudio"
            | "openai_compatible" => self.call_openai_compat(system, user, temperature).await,
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
