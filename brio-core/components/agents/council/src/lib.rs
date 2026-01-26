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
        // Council initializes by subscribing to objectives
        brio::core::pub_sub::subscribe("request:objective").map_err(|e| e.to_string())?;

        // Log startup
        brio::core::logging::log(
            brio::core::logging::Level::Info,
            "council-agent",
            &format!("Council initialized for task: {}", context.task_id),
        );

        Ok("Council Agent: Online and listening for objectives.".to_string())
    }
}

impl exports::brio::core::event_handler::Guest for Component {
    fn handle_event(topic: String, data: exports::brio::core::event_handler::Payload) {
        if topic == "request:objective" {
            let objective = match data {
                exports::brio::core::event_handler::Payload::Json(s) => s,
                _ => return, // Ignore binary for now
            };

            brio::core::logging::log(
                brio::core::logging::Level::Info,
                "council-agent",
                &format!("Received objective: {}", objective),
            );

            // Strategic Logic (Simulated for Prototype)
            // In a real system, this would call `brio::core::inference::chat(...)`

            let milestones = vec![
                "Phase 1: Setup Workspace",
                "Phase 2: Implement Core Login",
                "Phase 3: Verify Implementation",
            ];

            // Serialize milestones (manual JSON for prototype simplicity)
            // Structure: { "objective": "...", "milestones": ["...", ...] }
            let payload_json = format!(
                r#"{{"objective":"{}","milestones":[{}]}}"#,
                objective,
                milestones
                    .iter()
                    .map(|m| format!("\"{}\"", m))
                    .collect::<Vec<_>>()
                    .join(",")
            );

            // Publish proposal
            let _ = brio::core::pub_sub::publish(
                "proposal:milestones",
                &brio::core::pub_sub::Payload::Json(payload_json),
            );

            brio::core::logging::log(
                brio::core::logging::Level::Info,
                "council-agent",
                "Published proposal:milestones",
            );
        }
    }
}

export!(Component);
