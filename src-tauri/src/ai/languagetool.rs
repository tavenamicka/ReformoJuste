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
    matches: Vec<LtMatch>,
}

#[derive(Deserialize)]
struct LtMatch {
    offset: usize,
    length: usize,
    replacements: Vec<LtReplacement>,
}

#[derive(Deserialize)]
struct LtReplacement {
    value: String,
}

// ── Correction applier ────────────────────────────────────────────────────────
// LT offsets are Unicode code-point positions; we work on a char Vec to avoid
// byte-offset confusion with multi-byte characters.

fn apply_corrections(text: &str, matches: &[LtMatch]) -> String {
    let mut chars: Vec<char> = text.chars().collect();

    // Apply from end to start so earlier offsets stay valid after each splice.
    let mut sorted: Vec<&LtMatch> = matches.iter()
        .filter(|m| !m.replacements.is_empty())
        .collect();
    sorted.sort_by(|a, b| b.offset.cmp(&a.offset));

    for m in sorted {
        let start = m.offset;
        let end   = (m.offset + m.length).min(chars.len());
        let replacement: Vec<char> = m.replacements[0].value.chars().collect();
        chars.splice(start..end, replacement);
    }
    chars.into_iter().collect()
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
                        simple:       String::new(),
                        professional: String::new(),
                        formal:       String::new(),
                        short:        String::new(),
                        creative:     String::new(),
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
