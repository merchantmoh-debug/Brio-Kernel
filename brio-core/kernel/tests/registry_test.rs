use brio_kernel::engine::linker::create_engine_config;
use brio_kernel::registry::PluginRegistry;
use std::fs::File;
use tempfile::tempdir;
use wasmtime::Engine;

#[tokio::test]
async fn test_registry_scanning() -> anyhow::Result<()> {
    // Setup
    let dir = tempdir()?;
    let plugins_path = dir.path();

    // Create dummy plugins
    File::create(plugins_path.join("agent_alpha.wasm"))?;
    File::create(plugins_path.join("agent_beta.wasm"))?;
    File::create(plugins_path.join("README.txt"))?; // Should be ignored

    // Initialize Registry
    let config = create_engine_config();
    let engine = Engine::new(&config)?;
    let mut registry = PluginRegistry::new(engine);

    // Test load
    registry.load_from_directory(plugins_path).await?;

    let plugins = registry.list_plugins();
    assert_eq!(plugins.len(), 2, "Should find exactly 2 wasm files");

    let names: Vec<String> = plugins.iter().map(|p| p.id.clone()).collect();
    assert!(names.contains(&"agent_alpha".to_string()));
    assert!(names.contains(&"agent_beta".to_string()));

    Ok(())
}
