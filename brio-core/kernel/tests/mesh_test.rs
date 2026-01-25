use brio_kernel::host::BrioHostState;
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
            content: "".to_string(),
            usage: None,
        })
    }
}

#[tokio::test]
async fn test_mesh_routing() {
    // 1. Initialize Host State with in-memory DB
    let state = BrioHostState::with_provider("sqlite::memory:", Box::new(DummyProvider))
        .await
        .expect("Failed to create host");

    // 2. Create a mock agent channel
    let (tx, mut rx) = mpsc::channel::<MeshMessage>(10);

    // 3. Register the agent
    state.register_component("test-agent".to_string(), tx);

    // 4. Spawn a mock agent loop
    tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            match msg.method.as_str() {
                "echo" => {
                    // Echo the payload back
                    let _ = msg.reply_tx.send(Ok(msg.payload));
                }
                _ => {
                    let _ = msg.reply_tx.send(Err("Unknown method".to_string()));
                }
            }
        }
    });

    // 5. Test Happy Path: Echo
    let response = state
        .mesh_call(
            "test-agent",
            "echo",
            Payload::Json("Hello Brio".to_string()),
        )
        .await
        .expect("Mesh call failed");

    if let Payload::Json(s) = response {
        assert_eq!(s, "Hello Brio");
    } else {
        panic!("Expected Json payload");
    }

    // 6. Test Error Path: Unknown Method
    let err_response = state
        .mesh_call("test-agent", "bad_method", Payload::Json("".to_string()))
        .await;

    assert!(err_response.is_err());
    assert_eq!(
        err_response.unwrap_err().to_string(),
        "Target 'test-agent' returned error: Unknown method"
    );

    // 7. Test Missing Target
    let missing_response = state
        .mesh_call("ghost", "boo", Payload::Json("".to_string()))
        .await;

    assert!(missing_response.is_err());
    assert!(
        missing_response
            .unwrap_err()
            .to_string()
            .contains("not found")
    );
}
