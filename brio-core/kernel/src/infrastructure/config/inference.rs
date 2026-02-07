//! Inference provider configuration for the Brio kernel.
//!
//! This module defines AI/LLM API settings.

use secrecy::SecretString;
use serde::Deserialize;

/// Inference provider settings.
#[derive(Debug, Deserialize, Clone)]
pub struct InferenceSettings {
    /// `OpenAI` API key.
    pub openai_api_key: Option<SecretString>,
    /// `Anthropic` API key.
    pub anthropic_api_key: Option<SecretString>,
    /// Base URL for `OpenAI` API.
    pub openai_base_url: Option<String>,
}
