use wit_bindgen::generate;

generate!({
    world: "smart-agent",
    path: "../../../wit",
    skip: ["tool"],
    generate_all,
});

struct Component;

impl exports::brio::core::agent_runner::Guest for Component {
    fn run(context: exports::brio::core::agent_runner::TaskContext) -> Result<String, String> {
        let _ = context; // abstract for now
        // Coder agent implementation logic would go here
        Ok("Coder Agent: Ready to generate code.".to_string())
    }
}

export!(Component);
