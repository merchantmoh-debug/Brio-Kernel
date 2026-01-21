use brio_kernel::host::BrioHostState;
use brio_kernel::inference::ProviderRegistry;
use brio_kernel::mesh::service::MeshService;
use brio_kernel::mesh::types::{NodeId, NodeInfo, NodeAddress};
use brio_kernel::mesh::Payload;
use tokio::sync::mpsc;
use std::sync::Arc;
use std::time::Duration;

// Helper to spawn a node
async fn spawn_node(id: &str, port: u16) -> (Arc<BrioHostState>, String) {
    let node_id = NodeId::from(id.to_string());
    let addr_str = format!("127.0.0.1:{}", port);
    
    // Setup registry
    let registry = ProviderRegistry::new();
    
    // In-memory DB for tests
    let db_url = "sqlite::memory:";
    
    let state = BrioHostState::new_distributed(db_url, registry, node_id.clone())
        .await
        .expect("Failed to create host state");
    let state = Arc::new(state);
    
    // Spawn server
    let state_clone = state.clone();
    let addr = addr_str.parse().unwrap();
    let service = MeshService::new(state_clone, node_id);
    
    tokio::spawn(async move {
        tonic::transport::Server::builder()
            .add_service(brio_kernel::mesh::grpc::mesh_transport_server::MeshTransportServer::new(service))
            .serve(addr)
            .await
            .unwrap();
    });
    
    // Give server a moment to start
    tokio::time::sleep(Duration::from_millis(100)).await;
    
    (state, addr_str)
}

#[tokio::test]
async fn test_distributed_call() {
    // 1. Spawn two nodes
    let (node_a, _addr_a) = spawn_node("node-a", 50055).await;
    let (node_b, addr_b) = spawn_node("node-b", 50056).await;
    
    // 2. Register "echo" component on Node B
    let (tx, mut rx) = mpsc::channel(1);
    node_b.register_component("echo".to_string(), tx);
    
    // Handle echo requests on Node B
    tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            let reply = match msg.payload {
                Payload::Json(s) => Payload::Json(format!("echo: {}", s)),
                _ => Payload::Json("error".to_string()),
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
    
    // 4. Perform mesh call from A -> B
    // Target: "node-b/echo"
    let response = node_a.mesh_call("node-b/echo", "ping", Payload::Json("hello".to_string()))
        .await
        .expect("Mesh call failed");
        
    // 5. Verify response
    match response {
        Payload::Json(s) => assert_eq!(s, "echo: hello"),
        _ => panic!("Unexpected payload type"),
    }
}
