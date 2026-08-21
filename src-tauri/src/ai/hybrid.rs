use anyhow::Result;
use async_trait::async_trait;

use super::{AiProvider, AiResult};

/// Runs LanguageTool and an AI provider in parallel.
/// Takes `correction` from LanguageTool; the 5 reformulations from the AI.
pub struct HybridProvider {
    lt: Box<dyn AiProvider>,
    ai: Box<dyn AiProvider>,
}

impl HybridProvider {
    pub fn new(lt: Box<dyn AiProvider>, ai: Box<dyn AiProvider>) -> Self {
        Self { lt, ai }
    }
}

#[async_trait]
impl AiProvider for HybridProvider {
    async fn process(&self, text: &str) -> Result<AiResult> {
        let (lt_res, ai_res) = tokio::join!(
            self.lt.process(text),
            self.ai.process(text)
        );

        let mut result = ai_res?;

        // Override correction with LanguageTool result if available
        if let Ok(lt) = lt_res {
            result.correction = lt.correction;
        }

        Ok(result)
    }
}
