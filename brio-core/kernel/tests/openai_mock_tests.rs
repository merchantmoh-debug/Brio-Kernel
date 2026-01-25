//! HTTP mock tests for the OpenAI provider.
//!
//! Uses wiremock to simulate various HTTP responses from the OpenAI API.

use brio_kernel::inference::{
    ChatRequest, InferenceError, LLMProvider, Message, OpenAIConfig, OpenAIProvider, Role,
};
use reqwest::Url;
use secrecy::SecretString;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

async fn create_provider_with_mock_server(server: &MockServer) -> OpenAIProvider {
    let config = OpenAIConfig::new(
        SecretString::new("test-api-key".into()),
        Url::parse(&format!("{}/", server.uri())).unwrap(),
    )
    .with_max_retries(0); // Disable retries for faster tests
    OpenAIProvider::new(config)
}

fn create_test_request() -> ChatRequest {
    ChatRequest {
        model: "gpt-4".to_string(),
        messages: vec![Message {
            role: Role::User,
            content: "Hello".to_string(),
        }],
    }
}

// =============================================================================
// Rate Limit Tests
// =============================================================================

#[tokio::test]
async fn test_rate_limit_returns_rate_limit_error() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .respond_with(ResponseTemplate::new(429).set_body_string("Rate limit exceeded"))
        .mount(&server)
        .await;

    let provider = create_provider_with_mock_server(&server).await;
    let request = create_test_request();

    let result = provider.chat(request).await;

    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), InferenceError::RateLimit));
}

// =============================================================================
// Server Error Tests
// =============================================================================

#[tokio::test]
async fn test_server_error_returns_provider_error() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .respond_with(ResponseTemplate::new(500).set_body_string("Internal Server Error"))
        .mount(&server)
        .await;

    let provider = create_provider_with_mock_server(&server).await;
    let request = create_test_request();

    let result = provider.chat(request).await;

    assert!(result.is_err());
    if let InferenceError::ProviderError(msg) = result.unwrap_err() {
        assert!(msg.contains("500"));
    } else {
        panic!("Expected ProviderError");
    }
}

#[tokio::test]
async fn test_service_unavailable_returns_provider_error() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .respond_with(ResponseTemplate::new(503).set_body_string("Service Unavailable"))
        .mount(&server)
        .await;

    let provider = create_provider_with_mock_server(&server).await;
    let request = create_test_request();

    let result = provider.chat(request).await;

    assert!(result.is_err());
    if let InferenceError::ProviderError(msg) = result.unwrap_err() {
        assert!(msg.contains("503"));
    } else {
        panic!("Expected ProviderError");
    }
}

// =============================================================================
// Malformed Response Tests
// =============================================================================

#[tokio::test]
async fn test_malformed_json_returns_provider_error() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_string("this is not json"))
        .mount(&server)
        .await;

    let provider = create_provider_with_mock_server(&server).await;
    let request = create_test_request();

    let result = provider.chat(request).await;

    assert!(result.is_err());
    if let InferenceError::ProviderError(msg) = result.unwrap_err() {
        assert!(msg.contains("Parse error"));
    } else {
        panic!("Expected ProviderError with parse error");
    }
}

#[tokio::test]
async fn test_empty_choices_returns_provider_error() {
    let server = MockServer::start().await;

    // Valid JSON but empty choices array
    let response_body = r#"{"choices": [], "usage": null}"#;

    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_string(response_body))
        .mount(&server)
        .await;

    let provider = create_provider_with_mock_server(&server).await;
    let request = create_test_request();

    let result = provider.chat(request).await;

    assert!(result.is_err());
    if let InferenceError::ProviderError(msg) = result.unwrap_err() {
        assert!(msg.contains("No choices"));
    } else {
        panic!("Expected ProviderError about no choices");
    }
}

// =============================================================================
// Context Length Exceeded Tests
// =============================================================================

#[tokio::test]
async fn test_context_length_exceeded_returns_correct_error() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .respond_with(
            ResponseTemplate::new(400)
                .set_body_string(r#"{"error": {"message": "context_length_exceeded"}}"#),
        )
        .mount(&server)
        .await;

    let provider = create_provider_with_mock_server(&server).await;
    let request = create_test_request();

    let result = provider.chat(request).await;

    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        InferenceError::ContextLengthExceeded
    ));
}

// =============================================================================
// Success Path Tests
// =============================================================================

#[tokio::test]
async fn test_successful_response_parses_correctly() {
    let server = MockServer::start().await;

    let response_body = r#"{
        "choices": [
            {
                "message": {
                    "role": "assistant",
                    "content": "Hello! How can I help you?"
                }
            }
        ],
        "usage": {
            "prompt_tokens": 10,
            "completion_tokens": 8,
            "total_tokens": 18
        }
    }"#;

    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_string(response_body))
        .mount(&server)
        .await;

    let provider = create_provider_with_mock_server(&server).await;
    let request = create_test_request();

    let result = provider.chat(request).await;

    assert!(result.is_ok());
    let response = result.unwrap();
    assert_eq!(response.content, "Hello! How can I help you?");
    assert!(response.usage.is_some());
    let usage = response.usage.unwrap();
    assert_eq!(usage.prompt_tokens, 10);
    assert_eq!(usage.completion_tokens, 8);
    assert_eq!(usage.total_tokens, 18);
}
