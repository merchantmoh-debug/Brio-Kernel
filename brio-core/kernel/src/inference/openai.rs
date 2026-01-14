use crate::inference::provider::LLMProvider;
use crate::inference::types::{ChatRequest, ChatResponse, InferenceError, Message, Usage};
use anyhow::Result;
use async_trait::async_trait;
use reqwest::{Client, StatusCode, Url};
use secrecy::{ExposeSecret, SecretString};
use serde::{Deserialize, Serialize};

// =============================================================================
// Internal DTOs (Provider Specific)
// =============================================================================

#[derive(Serialize)]
struct OpenAIChatRequest {
    model: String,
    messages: Vec<Message>,
}

#[derive(Deserialize)]
struct OpenAIChoice {
    message: Message,
}

#[derive(Deserialize)]
struct OpenAIUsage {
    prompt_tokens: u32,
    completion_tokens: u32,
    total_tokens: u32,
}

#[derive(Deserialize)]
struct OpenAIChatResponse {
    choices: Vec<OpenAIChoice>,
    usage: Option<OpenAIUsage>,
}

// =============================================================================
// Configuration
// =============================================================================

pub struct OpenAIConfig {
    pub api_key: SecretString,
    pub base_url: Url,
}

// =============================================================================
// Implementation
// =============================================================================

pub struct OpenAIProvider {
    client: Client,
    config: OpenAIConfig,
}

impl OpenAIProvider {
    pub fn new(config: OpenAIConfig) -> Self {
        // Design by Contract: Validate state on construction
        // (Url and SecretString types already enforced some validation)
        Self {
            client: Client::new(),
            config,
        }
    }
}

#[async_trait]
impl LLMProvider for OpenAIProvider {
    async fn chat(&self, request: ChatRequest) -> Result<ChatResponse, InferenceError> {
        // 1. Map Domain -> Provider DTO
        let provider_req = OpenAIChatRequest {
            model: request.model,
            messages: request.messages,
        };

        let url = self
            .config
            .base_url
            .join("chat/completions")
            .map_err(|e| InferenceError::ConfigError(format!("Invalid URL join: {}", e)))?;

        // 2. Execute Request
        let res = self
            .client
            .post(url)
            .header(
                "Authorization",
                format!("Bearer {}", self.config.api_key.expose_secret()),
            )
            .header("Content-Type", "application/json")
            .json(&provider_req)
            .send()
            .await
            .map_err(|e| InferenceError::NetworkError(e.to_string()))?;

        // 3. Handle Errors (Map HTTP Status -> Domain Error)
        match res.status() {
            StatusCode::OK => {
                let body: OpenAIChatResponse = res
                    .json()
                    .await
                    .map_err(|e| InferenceError::ProviderError(format!("Parse error: {}", e)))?;

                // 4. Map Response -> Domain DTO
                let choice = body.choices.first().ok_or_else(|| {
                    InferenceError::ProviderError("No choices returned".to_string())
                })?;

                Ok(ChatResponse {
                    content: choice.message.content.clone(),
                    usage: body.usage.map(|u| Usage {
                        prompt_tokens: u.prompt_tokens,
                        completion_tokens: u.completion_tokens,
                        total_tokens: u.total_tokens,
                    }),
                })
            }
            StatusCode::TOO_MANY_REQUESTS => Err(InferenceError::RateLimit),
            StatusCode::BAD_REQUEST => {
                let text = res.text().await.unwrap_or_default();
                if text.contains("context_length_exceeded") {
                    Err(InferenceError::ContextLengthExceeded)
                } else {
                    Err(InferenceError::ProviderError(format!(
                        "Bad Request: {}",
                        text
                    )))
                }
            }
            _ => {
                let status = res.status();
                let text = res.text().await.unwrap_or_default();
                Err(InferenceError::ProviderError(format!(
                    "HTTP {}: {}",
                    status, text
                )))
            }
        }
    }
}
