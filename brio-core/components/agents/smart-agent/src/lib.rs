use wit_bindgen::generate;

generate!({
    world: "smart-agent",
    path: "../../../wit",
});

struct Component;

impl exports::agent_runner::Guest for Component {
    fn run() -> Result<String, String> {
        Ok("Hello from Smart Agent!".to_string())
    }
}

export!(Component);
