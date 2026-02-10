//! Tests for the mesh networking and message routing system.
//!
//! Tests component registration, message routing between agents, and
//! error handling for unknown methods and missing components.

use brio_kernel::host::{BrioHostState, MeshHandler};
use brio_kernel::mesh::{MeshMessage, Payload};
use tokio::sync::mpsc;

// Mock Provider for testing
struct DummyProvider;
#[async_trait::async_trait]
impl brio_kernel::inference::LLMProvider for DummyProvider {
    async fn chat(
        &self,
        _request: brio_kernel::inference::ChatRequest,
    ) -> Result<brio_kernel::inference::ChatResponse, brio_kernel::inference::InferenceError> {
        Ok(brio_kernel::inference::ChatResponse {
            content: String::new(),
            usage: None,
        })
    }
}

#[tokio::test]
async fn mesh_should_route_calls_to_registered_components() {
    let state = BrioHostState::with_provider("sqlite::memory:", Box::new(DummyProvider))
        .await
        .expect("Failed to create host");

    let (tx, mut rx) = mpsc::channel::<MeshMessage>(10);

    state.register_component("test-agent".to_string(), tx);

    tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            match msg.method.as_str() {
                "echo" => {
                    let _ = msg.reply_tx.send(Ok(msg.payload));
                }
                _ => {
                    let _ = msg.reply_tx.send(Err("Unknown method".to_string()));
                }
            }
        }
    });

    // Test Happy Path: Echo
    let response = state
        .mesh_call(
            "test-agent",
            "echo",
            Payload::Json(Box::new("Hello Brio".to_string())),
        )
        .await
        .expect("Mesh call failed");

    if let Payload::Json(s) = response {
        assert_eq!(*s, "Hello Brio");
    } else {
        panic!("Expected Json payload");
    }

    let err_response = state
        .mesh_call("test-agent", "bad_method", Payload::Json(Box::default()))
        .await;

    assert!(err_response.is_err());
    assert_eq!(
        err_response.unwrap_err().to_string(),
        "Target 'test-agent' returned error: Unknown method"
    );

    let missing_response = state
        .mesh_call("ghost", "boo", Payload::Json(Box::default()))
        .await;

    assert!(missing_response.is_err());
    assert!(
        missing_response
            .unwrap_err()
            .to_string()
            .contains("not found")
    );
}
