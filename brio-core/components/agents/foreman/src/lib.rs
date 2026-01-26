use serde::Deserialize;
use wit_bindgen::generate;

generate!({
    world: "smart-agent",
    path: "../../../wit",
    skip: ["tool"],
    generate_all,
});

#[derive(Deserialize)]
struct MilestonesEvent {
    // objective: String, // unused
    milestones: Vec<String>,
}

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

            let event: MilestonesEvent = match serde_json::from_str(&json_str) {
                Ok(e) => e,
                Err(e) => {
                    brio::core::logging::log(
                        brio::core::logging::Level::Error,
                        "foreman-agent",
                        &format!("Parse error: {}", e),
                    );
                    return;
                }
            };

            brio::core::logging::log(
                brio::core::logging::Level::Info,
                "foreman-agent",
                &format!("Processing {} milestones", event.milestones.len()),
            );

            // Create tasks in DB
            for milestone in event.milestones {
                // Insert into tasks table
                // Schema: content, priority, status, ... (from host schema)
                let sql = "INSERT INTO tasks (content, priority, status) VALUES (?, 10, 'pending')";
                let params = vec![milestone.clone()];

                match brio::core::sql_state::execute(sql, &params) {
                    Ok(_) => brio::core::logging::log(
                        brio::core::logging::Level::Info,
                        "foreman-agent",
                        &format!("Created task: {}", milestone),
                    ),
                    Err(e) => brio::core::logging::log(
                        brio::core::logging::Level::Error,
                        "foreman-agent",
                        &format!("DB Error: {}", e),
                    ),
                }
            }
        }
    }
}

export!(Component);
