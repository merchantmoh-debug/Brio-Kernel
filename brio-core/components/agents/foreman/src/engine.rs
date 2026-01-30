use crate::tools::{CreateTaskTool, Tool};
use anyhow::{Context, Result};
use serde::Deserialize;

#[derive(Deserialize)]
struct MilestonesEvent {
    milestones: Vec<String>,
}

pub struct ForemanEngine {
    create_task_tool: Box<dyn Tool>,
}

impl ForemanEngine {
    pub fn new() -> Self {
        Self {
            create_task_tool: Box::new(CreateTaskTool),
        }
    }

    pub fn process_milestones(&self, payload_json: &str) -> Result<()> {
        let event: MilestonesEvent =
            serde_json::from_str(payload_json).context("Failed to parse MilestonesEvent")?;

        crate::brio::core::logging::log(
            crate::brio::core::logging::Level::Info,
            "foreman-agent",
            &format!("Processing {} milestones", event.milestones.len()),
        );

        for milestone in event.milestones {
            crate::brio::core::logging::log(
                crate::brio::core::logging::Level::Info,
                "foreman-agent",
                &format!(
                    "Executing tool: {} for milestone '{}'",
                    self.create_task_tool.name(),
                    milestone
                ),
            );

            match self.create_task_tool.execute(&milestone) {
                Ok(msg) => crate::brio::core::logging::log(
                    crate::brio::core::logging::Level::Info,
                    "foreman-agent",
                    &msg,
                ),
                Err(e) => crate::brio::core::logging::log(
                    crate::brio::core::logging::Level::Error,
                    "foreman-agent",
                    &format!("Failed to create task for milestone '{}': {}", milestone, e),
                ),
            }
        }

        Ok(())
    }
}
