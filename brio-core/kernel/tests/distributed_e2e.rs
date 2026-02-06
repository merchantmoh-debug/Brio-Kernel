//! Distributed end-to-end tests for the mesh networking layer.
//!
//! Tests multi-node communication and service discovery across different nodes
//! in a distributed setup using gRPC transport.

use brio_kernel::host::BrioHostState;
use brio_kernel::inference::ProviderRegistry;
use brio_kernel::infrastructure::config::SandboxSettings;
use brio_kernel::mesh::Payload;
use brio_kernel::mesh::service::MeshService;
use brio_kernel::mesh::types::{NodeAddress, NodeId, NodeInfo};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;

/// Wait for server to be ready with exponential backoff retry loop
async fn wait_for_server(addr: &std::net::SocketAddr, max_retries: u64) -> bool {
    for i in 0..max_retries {
        if tokio::net::TcpStream::connect(addr).await.is_ok() {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(10 * (i + 1))).await;
    }
    false
}

// Helper to spawn a node
async fn spawn_node(id: &str, port: u16) -> (Arc<BrioHostState>, String) {
    let node_id = NodeId::from(id.to_string());
    let addr_str = format!("127.0.0.1:{port}");

    // Setup registry
    let registry = ProviderRegistry::new();

    // In-memory DB for tests
    let db_url = "sqlite::memory:";

    let host_state = BrioHostState::new_distributed(
        db_url,
        registry,
        None,
        node_id.clone(),
        SandboxSettings::default(),
    )
    .await
    .expect("Failed to create host state");
    let state = Arc::new(host_state);

    // Spawn server
    let state_clone = state.clone();
    let addr = addr_str.parse().unwrap();
    let service = MeshService::new(state_clone, node_id);

    tokio::spawn(async move {
        tonic::transport::Server::builder()
            .add_service(
                brio_kernel::mesh::grpc::mesh_transport_server::MeshTransportServer::new(service),
            )
            .serve(addr)
            .await
            .unwrap();
    });

    // Wait for server to be ready with retry loop
    assert!(
        wait_for_server(&addr, 10).await,
        "Server failed to start within timeout"
    );

    (state, addr_str)
}

#[tokio::test]
async fn test_distributed_call() {
    let (node_a, _addr_a) = spawn_node("node-a", 50055).await;
    let (node_b, addr_b) = spawn_node("node-b", 50056).await;

    // Register "echo" component on Node B
    let (tx, mut rx) = mpsc::channel(1);
    node_b.register_component("echo".to_string(), tx);

    // Handle echo requests on Node B
    tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            let reply = match msg.payload {
                Payload::Json(s) => Payload::Json(Box::new(format!("echo: {s}"))),
                Payload::Binary(_) => Payload::Json(Box::new("error".to_string())),
            };
            msg.reply_tx.send(Ok(reply)).unwrap();
        }
    });

    // 3. Tell Node A about Node B (Manual Discovery)
    let info_b = NodeInfo {
        id: NodeId::from("node-b".to_string()),
        address: NodeAddress(addr_b),
        capabilities: vec![],
        last_seen: 0,
    };
    node_a.register_remote_node(info_b);

    let response = node_a
        .mesh_call(
            "node-b/echo",
            "ping",
            Payload::Json(Box::new("hello".to_string())),
        )
        .await
        .expect("Mesh call failed");
    match response {
        Payload::Json(s) => assert_eq!(*s, "echo: hello"),
        Payload::Binary(_) => panic!("Unexpected payload type"),
    }
}
