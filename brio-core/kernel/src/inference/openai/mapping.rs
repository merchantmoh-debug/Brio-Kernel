//! `OpenAI` API type mapping.
//!
//! This module provides types for mapping between internal and `OpenAI` API formats.

use crate::inference::types::{ChatResponse, Message, Usage};
use serde::{Deserialize, Serialize};

/// `OpenAI` API chat request format
#[derive(Serialize)]
pub struct OpenAIChatRequest {
    /// The model identifier (e.g., "gpt-4", "gpt-3.5-turbo")
    pub model: String,
    /// The conversation messages
    pub messages: Vec<Message>,
}

/// `OpenAI` API choice structure
#[derive(Deserialize)]
pub struct OpenAIChoice {
    /// The generated message
    pub message: Message,
}

/// `OpenAI` API usage information
#[derive(Deserialize)]
pub struct OpenAIUsage {
    /// Number of tokens in the prompt
    #[serde(rename = "prompt_tokens")]
    pub prompt: u32,
    /// Number of tokens in the completion
    #[serde(rename = "completion_tokens")]
    pub completion: u32,
    /// Total number of tokens used
    #[serde(rename = "total_tokens")]
    pub total: u32,
}

/// `OpenAI` API chat response format
#[derive(Deserialize)]
pub struct OpenAIChatResponse {
    /// The generated completion choices
    pub choices: Vec<OpenAIChoice>,
    /// Token usage information if available
    pub usage: Option<OpenAIUsage>,
}

/// Maps `OpenAI` API response to internal `ChatResponse`
///
/// # Errors
///
/// Returns an error if no choices are returned in the response.
pub fn map_response(body: OpenAIChatResponse) -> Result<ChatResponse, String> {
    let choice = body
        .choices
        .first()
        .ok_or_else(|| "No choices returned".to_string())?;

    Ok(ChatResponse {
        content: choice.message.content.clone(),
        usage: body.usage.map(|u| Usage {
            prompt_tokens: u.prompt,
            completion_tokens: u.completion,
            total_tokens: u.total,
        }),
    })
}

/// Creates an `OpenAI` API request from internal types
#[must_use]
pub fn create_request(model: String, messages: Vec<Message>) -> OpenAIChatRequest {
    OpenAIChatRequest { model, messages }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inference::types::{Message, Role};

    #[test]
    fn test_create_request() {
        let messages = vec![
            Message {
                role: Role::User,
                content: "Hello".to_string(),
            },
            Message {
                role: Role::Assistant,
                content: "Hi there".to_string(),
            },
        ];

        let request = create_request("gpt-4".to_string(), messages);
        assert_eq!(request.model, "gpt-4");
        assert_eq!(request.messages.len(), 2);
    }

    #[test]
    fn test_map_response_success() {
        let body = OpenAIChatResponse {
            choices: vec![OpenAIChoice {
                message: Message {
                    role: Role::Assistant,
                    content: "Test response".to_string(),
                },
            }],
            usage: Some(OpenAIUsage {
                prompt: 10,
                completion: 20,
                total: 30,
            }),
        };

        let response = map_response(body).unwrap();
        assert_eq!(response.content, "Test response");
        assert!(response.usage.is_some());
        let usage = response.usage.unwrap();
        assert_eq!(usage.prompt_tokens, 10);
        assert_eq!(usage.completion_tokens, 20);
        assert_eq!(usage.total_tokens, 30);
    }

    #[test]
    fn test_map_response_no_choices() {
        let body = OpenAIChatResponse {
            choices: vec![],
            usage: None,
        };

        let result = map_response(body);
        assert!(result.is_err());
    }

    #[test]
    fn test_map_response_no_usage() {
        let body = OpenAIChatResponse {
            choices: vec![OpenAIChoice {
                message: Message {
                    role: Role::Assistant,
                    content: "No usage".to_string(),
                },
            }],
            usage: None,
        };

        let response = map_response(body).unwrap();
        assert_eq!(response.content, "No usage");
        assert!(response.usage.is_none());
    }
}
