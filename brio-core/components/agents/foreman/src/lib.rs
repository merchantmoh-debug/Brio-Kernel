//! Foreman Agent - A task orchestration agent.
//!
//! This agent uses the agent-sdk for task orchestration using pub-sub events.
//! Unlike other agents, it's event-driven and subscribes to milestone proposals.

use agent_sdk::Tool;
use anyhow::Result;
use serde::Deserialize;
use std::collections::HashMap;
use wit_bindgen::generate;

// Generate WIT bindings at crate root level
generate!({
    world: "smart-agent",
    path: "../../../wit",
    skip: ["tool"],
    generate_all,
});

/// ForemanAgent implements the agent-runner and event-handler interfaces.
pub struct ForemanAgent;

impl exports::brio::core::agent_runner::Guest for ForemanAgent {
    fn run(context: exports::brio::core::agent_runner::TaskContext) -> Result<String, String> {
        // Subscribe to proposal milestones topic
        brio::core::pub_sub::subscribe("proposal:milestones").map_err(|e| e.to_string())?;

        brio::core::logging::log(
            brio::core::logging::Level::Info,
            "foreman-agent",
            &format!("Foreman initialized for task: {}", context.task_id),
        );

        Ok("Foreman Agent: Monitoring proposals.".to_string())
    }
}

impl exports::brio::core::event_handler::Guest for ForemanAgent {
    fn handle_event(topic: String, data: exports::brio::core::event_handler::Payload) {
        let data_str = match &data {
            exports::brio::core::event_handler::Payload::Json(s) => s.clone(),
            exports::brio::core::event_handler::Payload::Binary(_) => {
                brio::core::logging::log(
                    brio::core::logging::Level::Warn,
                    "foreman-agent",
                    "Received binary payload, expected JSON",
                );
                return;
            }
        };

        if topic == "proposal:milestones" {
            let engine = ForemanEngine::new();
            if let Err(e) = engine.process_milestones(&data_str) {
                brio::core::logging::log(
                    brio::core::logging::Level::Error,
                    "foreman-agent",
                    &format!("Error processing milestones: {}", e),
                );
            }
        } else {
            brio::core::logging::log(
                brio::core::logging::Level::Debug,
                "foreman-agent",
                &format!("Received event on unhandled topic: {}", topic),
            );
        }
    }
}

/// Event payload for milestone proposals.
#[derive(Deserialize, Debug)]
struct MilestonesEvent {
    /// List of milestone descriptions.
    milestones: Vec<String>,
}

/// Engine for processing foreman tasks.
pub struct ForemanEngine {
    create_task_tool: Box<dyn Tool>,
}

impl ForemanEngine {
    /// Creates a new foreman engine with the create task tool.
    pub fn new() -> Self {
        Self {
            create_task_tool: Box::new(CreateTaskTool),
        }
    }

    /// Processes milestones from an event payload.
    ///
    /// # Errors
    ///
    /// Returns an error if the payload cannot be parsed or if task creation fails.
    pub fn process_milestones(
        &self,
        payload_json: &str,
    ) -> Result<(), agent_sdk::error::ToolError> {
        let event: MilestonesEvent = serde_json::from_str(payload_json).map_err(|e| {
            agent_sdk::error::ToolError::InvalidArguments {
                tool: "process_milestones".to_string(),
                reason: format!("Failed to parse MilestonesEvent: {}", e),
            }
        })?;

        brio::core::logging::log(
            brio::core::logging::Level::Info,
            "foreman-agent",
            &format!("Processing {} milestones", event.milestones.len()),
        );

        for milestone in event.milestones {
            brio::core::logging::log(
                brio::core::logging::Level::Info,
                "foreman-agent",
                &format!(
                    "Executing tool: {} for milestone '{}'",
                    self.create_task_tool.name(),
                    milestone
                ),
            );

            let mut args = HashMap::new();
            args.insert("milestone".to_string(), milestone.clone());

            match self.create_task_tool.execute(args) {
                Ok(msg) => {
                    brio::core::logging::log(
                        brio::core::logging::Level::Info,
                        "foreman-agent",
                        &msg,
                    );
                }
                Err(e) => {
                    brio::core::logging::log(
                        brio::core::logging::Level::Error,
                        "foreman-agent",
                        &format!("Failed to create task for milestone '{}': {}", milestone, e),
                    );
                }
            }
        }

        Ok(())
    }
}

impl Default for ForemanEngine {
    fn default() -> Self {
        Self::new()
    }
}

/// Tool for creating tasks from milestones.
pub struct CreateTaskTool;

impl Tool for CreateTaskTool {
    fn name(&self) -> &str {
        "create_task"
    }

    fn description(&self) -> &str {
        "Creates a task from a milestone description in the task database"
    }

    fn execute(
        &self,
        args: HashMap<String, String>,
    ) -> Result<String, agent_sdk::error::ToolError> {
        let milestone =
            args.get("milestone")
                .ok_or_else(|| agent_sdk::error::ToolError::InvalidArguments {
                    tool: "create_task".to_string(),
                    reason: "Missing 'milestone' argument".to_string(),
                })?;

        let sql = "INSERT INTO tasks (content, priority, status) VALUES (?, 10, 'pending')";
        let params = vec![milestone.to_string()];

        brio::core::sql_state::execute(sql, &params)
            .map(|_| format!("Created task: {}", milestone))
            .map_err(|e| agent_sdk::error::ToolError::ExecutionFailed {
                tool: "create_task".to_string(),
                source: Box::new(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    format!("DB Error: {}", e),
                )),
            })
    }
}

export!(ForemanAgent);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_milestones_event_deserialization() {
        let json = r#"{"milestones": ["Milestone 1", "Milestone 2"]}"#;
        let event: MilestonesEvent = serde_json::from_str(json).unwrap();
        assert_eq!(event.milestones.len(), 2);
        assert_eq!(event.milestones[0], "Milestone 1");
        assert_eq!(event.milestones[1], "Milestone 2");
    }

    #[test]
    fn test_create_task_tool_name() {
        let tool = CreateTaskTool;
        assert_eq!(tool.name(), "create_task");
    }
}
