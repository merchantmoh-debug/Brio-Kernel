use wit_bindgen::generate;

generate!({
    world: "smart-agent",
    path: "../../../wit",
    skip: ["tool"],
    generate_all,
});

mod engine;
mod tools;

use engine::ForemanEngine;

struct Component;

impl exports::brio::core::agent_runner::Guest for Component {
    fn run(context: exports::brio::core::agent_runner::TaskContext) -> Result<String, String> {
        brio::core::pub_sub::subscribe("proposal:milestones").map_err(|e| e.to_string())?;

        brio::core::logging::log(
            brio::core::logging::Level::Info,
            "foreman-agent",
            &format!("Foreman initialized for task: {}", context.task_id),
        );

        Ok("Foreman Agent: Monitoring proposals.".to_string())
    }
}

impl exports::brio::core::event_handler::Guest for Component {
    fn handle_event(topic: String, data: exports::brio::core::event_handler::Payload) {
        if topic == "proposal:milestones" {
            let json_str = match data {
                exports::brio::core::event_handler::Payload::Json(s) => s,
                _ => return,
            };

            let engine = ForemanEngine::new();
            if let Err(e) = engine.process_milestones(&json_str) {
                brio::core::logging::log(
                    brio::core::logging::Level::Error,
                    "foreman-agent",
                    &format!("Error processing milestones: {}", e),
                );
            }
        }
    }
}

export!(Component);
