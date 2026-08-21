use anyhow::Result;
use async_trait::async_trait;
use reqwest::{Client, StatusCode};
use serde_json::json;

use super::{build_prompt, parse_response, AiProvider, AiResult};

const API_URL: &str = "https://api.mistral.ai/v1/chat/completions";
const MODELS_URL: &str = "https://api.mistral.ai/v1/models";

const SYSTEM_PROMPT: &str = "Tu es un correcteur orthographique et grammatical en FRANÇAIS. \
Tu corriges les fautes d'orthographe, de grammaire et de conjugaison sans changer le sens du texte, \
puis tu proposes cinq reformulations (simple, professionnelle, soutenue, courte et créative). \
Tu réponds toujours en français et UNIQUEMENT avec un objet JSON valide.";

/// Provider basé sur l'API REST de Mistral (cloud).
pub struct MistralProvider {
    api_key: String,
    model:   String,
    client:  Client,
}

impl MistralProvider {
    pub fn new(api_key: String, model: String) -> Self {
        // Client partagé (pool de connexions + timeout) — cf. ai/mod.rs.
        Self { api_key, model, client: super::ai_client() }
    }

    /// Ping léger pour la détection automatique : vérifie clé + disponibilité.
    /// `false` si la clé est vide, si le réseau échoue ou si l'API rejette la clé.
    pub async fn ping(&self) -> bool {
        if self.api_key.is_empty() {
            return false;
        }
        match super::probe_client().get(MODELS_URL).bearer_auth(&self.api_key).send().await {
            Ok(resp) => resp.status().is_success(),
            Err(_) => false,
        }
    }
}

#[async_trait]
impl AiProvider for MistralProvider {
    async fn process(&self, text: &str) -> Result<AiResult> {
        if self.api_key.is_empty() {
            anyhow::bail!("Clé API Mistral absente (mistral_api_key vide)");
        }

        let body = json!({
            "model": self.model,
            "messages": [
                { "role": "system", "content": SYSTEM_PROMPT },
                { "role": "user",   "content": build_prompt(text) }
            ],
            "temperature": 0.7,
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
            StatusCode::UNAUTHORIZED => {
                anyhow::bail!("Mistral : clé API invalide ou révoquée (401)")
            }
            StatusCode::TOO_MANY_REQUESTS => {
                anyhow::bail!("Mistral : quota dépassé / trop de requêtes (429)")
            }
            s if !s.is_success() => {
                let detail = resp.text().await.unwrap_or_default();
                anyhow::bail!("Mistral : erreur HTTP {} — {}", s.as_u16(), detail)
            }
            _ => {}
        }

        let data: serde_json::Value = resp.json().await?;
        let content = data["choices"][0]["message"]["content"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Réponse Mistral vide ou malformée : {:?}", data))?;

        parse_response(content)
    }
}
