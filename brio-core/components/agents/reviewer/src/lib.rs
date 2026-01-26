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
        let _ = context;
        // Reviewer agent logic
        Ok("Reviewer Agent: Ready to review code.".to_string())
    }
}

export!(Component);
