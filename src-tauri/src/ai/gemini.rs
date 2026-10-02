use anyhow::Result;
use async_trait::async_trait;
use reqwest::{Client, StatusCode};
use serde_json::json;

use super::{two_pass, AiProvider, AiResult, LlmProvider};

// Endpoint de compatibilité OpenAI de Google : même format que Mistral.
const API_URL:    &str = "https://generativelanguage.googleapis.com/v1beta/openai/chat/completions";
const MODELS_URL: &str = "https://generativelanguage.googleapis.com/v1beta/openai/models";

/// Provider basé sur l'API Google Gemini (palier gratuit avec quotas).
pub struct GeminiProvider {
    api_key: String,
    model:   String,
    client:  Client,
}

impl GeminiProvider {
    pub fn new(api_key: String, model: String) -> Self {
        Self { api_key, model, client: super::ai_client() }
    }

    /// Ping léger pour la détection automatique : `false` si clé vide,
    /// réseau en échec ou clé rejetée.
    pub async fn ping(&self) -> bool {
        if self.api_key.is_empty() {
            return false;
        }
        match super::probe_client().get(MODELS_URL).bearer_auth(&self.api_key).send().await {
            Ok(resp) => resp.status().is_success(),
            Err(_)   => false,
        }
    }
}

#[async_trait]
impl LlmProvider for GeminiProvider {
    async fn complete(&self, system: &str, user: &str, temperature: f32) -> Result<String> {
        if self.api_key.is_empty() {
            anyhow::bail!("Clé API Gemini absente (gemini_api_key vide)");
        }

        let body = json!({
            "model": self.model,
            "messages": [
                { "role": "system", "content": system },
                { "role": "user",   "content": user }
            ],
            "temperature": temperature,
            "response_format": { "type": "json_object" }
        });

        let resp = self.client
            .post(API_URL)
            .bearer_auth(&self.api_key)
            .json(&body)
            .send()
            .await?;

        let status = resp.status();
        match status {
            StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => {
                anyhow::bail!("Gemini : clé API invalide ou non autorisée ({})", status.as_u16())
            }
            StatusCode::TOO_MANY_REQUESTS => {
                anyhow::bail!("Gemini : quota gratuit dépassé (429)")
            }
            s if !s.is_success() => {
                // Gemini renvoie 400 (et non 401) pour une clé invalide.
                let detail = resp.text().await.unwrap_or_default();
                anyhow::bail!("Gemini : erreur HTTP {} — {}", s.as_u16(), detail)
            }
            _ => {}
        }

        let data: serde_json::Value = resp.json().await?;
        data["choices"][0]["message"]["content"]
            .as_str()
            .map(str::to_string)
            .ok_or_else(|| anyhow::anyhow!("Réponse Gemini vide ou malformée : {:?}", data))
    }
}

#[async_trait]
impl AiProvider for GeminiProvider {
    async fn process(&self, text: &str) -> Result<AiResult> {
        two_pass(self, text).await
    }
}
