//! Tests for the inference module types and error handling.

use brio_kernel::inference::{
    ChatRequest, ChatResponse, InferenceError, LLMProvider, Message, Role, Usage,
};

// =============================================================================
// Type Tests
// =============================================================================

#[test]
fn test_role_variants() {
    let system = Role::System;
    let user = Role::User;
    let assistant = Role::Assistant;

    // Ensure all variants are distinct
    assert_ne!(format!("{:?}", system), format!("{:?}", user));
    assert_ne!(format!("{:?}", user), format!("{:?}", assistant));
}

#[test]
fn test_message_construction() {
    let msg = Message {
        role: Role::User,
        content: "Hello, world!".to_string(),
    };

    assert!(matches!(msg.role, Role::User));
    assert_eq!(msg.content, "Hello, world!");
}

#[test]
fn test_usage_struct() {
    let usage = Usage {
        prompt_tokens: 10,
        completion_tokens: 20,
        total_tokens: 30,
    };

    assert_eq!(usage.prompt_tokens, 10);
    assert_eq!(usage.completion_tokens, 20);
    assert_eq!(usage.total_tokens, 30);
}

#[test]
fn test_chat_request_construction() {
    let request = ChatRequest {
        model: "gpt-4".to_string(),
        messages: vec![
            Message {
                role: Role::System,
                content: "You are helpful.".to_string(),
            },
            Message {
                role: Role::User,
                content: "Hi!".to_string(),
            },
        ],
    };

    assert_eq!(request.model, "gpt-4");
    assert_eq!(request.messages.len(), 2);
}

#[test]
fn test_chat_response_with_usage() {
    let response = ChatResponse {
        content: "Hello!".to_string(),
        usage: Some(Usage {
            prompt_tokens: 5,
            completion_tokens: 1,
            total_tokens: 6,
        }),
    };

    assert_eq!(response.content, "Hello!");
    assert!(response.usage.is_some());
    assert_eq!(response.usage.unwrap().total_tokens, 6);
}

#[test]
fn test_chat_response_without_usage() {
    let response = ChatResponse {
        content: "Response".to_string(),
        usage: None,
    };

    assert!(response.usage.is_none());
}

// =============================================================================
// Error Tests
// =============================================================================

#[test]
fn test_inference_error_provider_display() {
    let err = InferenceError::ProviderError("API failed".to_string());
    let display = format!("{}", err);
    assert!(display.contains("Provider Error"));
    assert!(display.contains("API failed"));
}

#[test]
fn test_inference_error_rate_limit_display() {
    let err = InferenceError::RateLimit;
    let display = format!("{}", err);
    assert!(display.contains("Rate Limit"));
}

#[test]
fn test_inference_error_context_length_display() {
    let err = InferenceError::ContextLengthExceeded;
    let display = format!("{}", err);
    assert!(display.contains("Context Length"));
}

#[test]
fn test_inference_error_network_display() {
    let err = InferenceError::NetworkError("Connection refused".to_string());
    let display = format!("{}", err);
    assert!(display.contains("Network Error"));
    assert!(display.contains("Connection refused"));
}

#[test]
fn test_inference_error_config_display() {
    let err = InferenceError::ConfigError("Invalid URL".to_string());
    let display = format!("{}", err);
    assert!(display.contains("Configuration Error"));
    assert!(display.contains("Invalid URL"));
}

// =============================================================================
// Serialization Tests
// =============================================================================

#[test]
fn test_role_serialization() {
    let system = Role::System;
    let user = Role::User;
    let assistant = Role::Assistant;

    // Verify serde serialization produces expected lowercase values
    let system_json = serde_json::to_string(&system).unwrap();
    let user_json = serde_json::to_string(&user).unwrap();
    let assistant_json = serde_json::to_string(&assistant).unwrap();

    assert_eq!(system_json, r#""system""#);
    assert_eq!(user_json, r#""user""#);
    assert_eq!(assistant_json, r#""assistant""#);
}

#[test]
fn test_message_serialization() {
    let msg = Message {
        role: Role::User,
        content: "Test message".to_string(),
    };

    let json = serde_json::to_string(&msg).unwrap();
    assert!(json.contains(r#""role":"user""#));
    assert!(json.contains(r#""content":"Test message""#));
}

#[test]
fn test_message_deserialization() {
    let json = r#"{"role":"assistant","content":"Hello!"}"#;
    let msg: Message = serde_json::from_str(json).unwrap();

    assert!(matches!(msg.role, Role::Assistant));
    assert_eq!(msg.content, "Hello!");
}

#[test]
fn test_usage_serialization_roundtrip() {
    let usage = Usage {
        prompt_tokens: 100,
        completion_tokens: 50,
        total_tokens: 150,
    };

    let json = serde_json::to_string(&usage).unwrap();
    let parsed: Usage = serde_json::from_str(&json).unwrap();

    assert_eq!(parsed.prompt_tokens, usage.prompt_tokens);
    assert_eq!(parsed.completion_tokens, usage.completion_tokens);
    assert_eq!(parsed.total_tokens, usage.total_tokens);
}

// =============================================================================
// Mock Provider Tests
// =============================================================================

struct TestMockProvider {
    response: String,
}

#[async_trait::async_trait]
impl LLMProvider for TestMockProvider {
    async fn chat(&self, _request: ChatRequest) -> Result<ChatResponse, InferenceError> {
        Ok(ChatResponse {
            content: self.response.clone(),
            usage: None,
        })
    }
}

#[tokio::test]
async fn test_mock_provider_returns_configured_response() {
    let provider = TestMockProvider {
        response: "Mocked response".to_string(),
    };

    let request = ChatRequest {
        model: "test-model".to_string(),
        messages: vec![Message {
            role: Role::User,
            content: "Hello".to_string(),
        }],
    };

    let response = provider.chat(request).await.unwrap();
    assert_eq!(response.content, "Mocked response");
}

struct FailingMockProvider;

#[async_trait::async_trait]
impl LLMProvider for FailingMockProvider {
    async fn chat(&self, _request: ChatRequest) -> Result<ChatResponse, InferenceError> {
        Err(InferenceError::ProviderError(
            "Simulated failure".to_string(),
        ))
    }
}

#[tokio::test]
async fn test_failing_provider_returns_error() {
    let provider = FailingMockProvider;

    let request = ChatRequest {
        model: "test-model".to_string(),
        messages: vec![],
    };

    let result = provider.chat(request).await;
    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        InferenceError::ProviderError(_)
    ));
}
