//! Tests for the BrioHostState and host functionality.

use anyhow::Result;
use brio_kernel::host::BrioHostState;
use brio_kernel::inference::{ChatRequest, ChatResponse, InferenceError, LLMProvider};
use brio_kernel::mesh::{MeshMessage, Payload};
use std::sync::Arc;
use tokio::sync::mpsc;

// =============================================================================
// Mock Provider
// =============================================================================

struct MockProvider;

#[async_trait::async_trait]
impl LLMProvider for MockProvider {
    async fn chat(&self, _request: ChatRequest) -> Result<ChatResponse, InferenceError> {
        Ok(ChatResponse {
            content: "Mock response".to_string(),
            usage: None,
        })
    }
}

// =============================================================================
// Host State Creation Tests
// =============================================================================

#[tokio::test]
async fn test_host_state_creation() -> Result<()> {
    let host = BrioHostState::with_provider("sqlite::memory:", Box::new(MockProvider)).await?;
    // Verify we can access the DB pool
    let _db = host.db();
    Ok(())
}

#[tokio::test]
async fn test_host_state_broadcaster_access() -> Result<()> {
    let host = BrioHostState::with_provider("sqlite::memory:", Box::new(MockProvider)).await?;
    let broadcaster = host.broadcaster();
    // Broadcaster should start with 0 clients
    assert_eq!(broadcaster.client_count(), 0);
    Ok(())
}

#[tokio::test]
async fn test_host_state_inference_access() -> Result<()> {
    let host = BrioHostState::with_provider("sqlite::memory:", Box::new(MockProvider)).await?;
    let inference = host.inference();
    // Should be able to clone the Arc
    let _cloned = inference.clone();
    Ok(())
}

// =============================================================================
// Component Registration Tests
// =============================================================================

#[tokio::test]
async fn test_register_and_call_component() -> Result<()> {
    let host =
        Arc::new(BrioHostState::with_provider("sqlite::memory:", Box::new(MockProvider)).await?);

    let (tx, mut rx) = mpsc::channel::<MeshMessage>(10);
    host.register_component("test-component".to_string(), tx);

    // Spawn mock component handler
    let host_clone = host.clone();
    tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            if msg.method == "ping" {
                let _ = msg.reply_tx.send(Ok(Payload::Json(Box::new("pong".to_string()))));
            }
        }
    });

    // Call the component
    let response = host_clone
        .mesh_call("test-component", "ping", Payload::Json(Box::new("".to_string())))
        .await?;

    if let Payload::Json(s) = response {
        assert_eq!(*s, "pong");
    } else {
        panic!("Expected Json payload");
    }

    Ok(())
}

#[tokio::test]
async fn test_mesh_call_to_missing_target() -> Result<()> {
    let host = BrioHostState::with_provider("sqlite::memory:", Box::new(MockProvider)).await?;

    let result = host
        .mesh_call("nonexistent", "method", Payload::Json(Box::new("".to_string())))
        .await;

    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("not found"));

    Ok(())
}

#[tokio::test]
async fn test_register_multiple_components() -> Result<()> {
    let host =
        Arc::new(BrioHostState::with_provider("sqlite::memory:", Box::new(MockProvider)).await?);

    let (tx1, mut rx1) = mpsc::channel::<MeshMessage>(10);
    let (tx2, mut rx2) = mpsc::channel::<MeshMessage>(10);

    host.register_component("component-1".to_string(), tx1);
    host.register_component("component-2".to_string(), tx2);

    // Handle component 1
    tokio::spawn(async move {
        while let Some(msg) = rx1.recv().await {
            let _ = msg.reply_tx.send(Ok(Payload::Json(Box::new("from-1".to_string()))));
        }
    });

    // Handle component 2
    tokio::spawn(async move {
        while let Some(msg) = rx2.recv().await {
            let _ = msg.reply_tx.send(Ok(Payload::Json(Box::new("from-2".to_string()))));
        }
    });

    // Call both
    let resp1 = host
        .mesh_call("component-1", "test", Payload::Json(Box::new("".to_string())))
        .await?;
    let resp2 = host
        .mesh_call("component-2", "test", Payload::Json(Box::new("".to_string())))
        .await?;

    if let Payload::Json(s) = resp1 {
        assert_eq!(*s, "from-1");
    }
    if let Payload::Json(s) = resp2 {
        assert_eq!(*s, "from-2");
    }

    Ok(())
}

// =============================================================================
// Store Tests
// =============================================================================

#[tokio::test]
async fn test_store() -> Result<()> {
    let host = BrioHostState::with_provider("sqlite::memory:", Box::new(MockProvider)).await?;
    let _store = host.store("test_scope");
    // Store should be created without error
    Ok(())
}

// =============================================================================
// Session Tests
// =============================================================================

#[tokio::test]
async fn test_session_with_nonexistent_path() -> Result<()> {
    let host = BrioHostState::with_provider("sqlite::memory:", Box::new(MockProvider)).await?;
    let result = host.begin_session("/nonexistent/path/12345");

    assert!(result.is_err());
    Ok(())
}

#[tokio::test]
async fn test_session_begin_and_commit() -> Result<()> {
    let host = BrioHostState::with_provider("sqlite::memory:", Box::new(MockProvider)).await?;

    // Create a temporary directory for the session
    let temp = std::env::temp_dir().join("brio_host_test_session");
    if temp.exists() {
        std::fs::remove_dir_all(&temp)?;
    }
    std::fs::create_dir_all(&temp)?;
    std::fs::write(temp.join("test.txt"), "hello")?;

    let session_id = host
        .begin_session(temp.to_str().unwrap())
        .unwrap();
    assert!(!session_id.is_empty());

    // Commit should succeed
    let commit_result = host.commit_session(&session_id);
    assert!(commit_result.is_ok());

    // Cleanup
    let _ = std::fs::remove_dir_all(&temp);

    Ok(())
}

// =============================================================================
// Broadcast Patch Test
// =============================================================================

#[tokio::test]
async fn test_broadcast_patch() -> Result<()> {
    use brio_kernel::ws::WsPatch;

    let host = BrioHostState::with_provider("sqlite::memory:", Box::new(MockProvider)).await?;

    // Create an empty patch
    let patch = WsPatch::new(json_patch::Patch(vec![]));

    // Broadcast should succeed even with no subscribers
    let result = host.broadcast_patch(patch);
    assert!(result.is_ok());

    Ok(())
}

// =============================================================================
// Binary Payload Test
// =============================================================================

#[tokio::test]
async fn test_binary_payload_routing() -> Result<()> {
    let host =
        Arc::new(BrioHostState::with_provider("sqlite::memory:", Box::new(MockProvider)).await?);

    let (tx, mut rx) = mpsc::channel::<MeshMessage>(10);
    host.register_component("binary-handler".to_string(), tx);

    tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            if let Payload::Binary(data) = msg.payload {
                // Echo back reversed
                let reversed: Vec<u8> = data.into_iter().rev().collect();
                let _ = msg.reply_tx.send(Ok(Payload::Binary(Box::new(reversed))));
            }
        }
    });

    let response = host
        .mesh_call(
            "binary-handler",
            "reverse",
            Payload::Binary(Box::new(vec![1, 2, 3, 4])),
        )
        .await?;

    if let Payload::Binary(data) = response {
        assert_eq!(*data, vec![4, 3, 2, 1]);
    } else {
        panic!("Expected Binary payload");
    }

    Ok(())
}
