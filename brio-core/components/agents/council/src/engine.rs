use crate::AgentState;
use crate::brio::ai::inference;
use crate::prompt::PromptBuilder;
use crate::tools::{DoneTool, ToolRegistry};
use anyhow::{Result, anyhow};

pub struct AgentEngine {
    state: AgentState,
    tools: ToolRegistry,
}

impl AgentEngine {
    pub fn new(context: crate::exports::brio::core::agent_runner::TaskContext) -> Self {
        // Initialize Tools
        let mut tools = ToolRegistry::new();
        // Council tools can be added here
        tools.register(Box::new(DoneTool));

        // Initialize Prompt
        let system_prompt = PromptBuilder::build(&context, &tools);
        let state = AgentState::new(system_prompt);

        Self { state, tools }
    }

    pub fn run(&mut self) -> Result<String> {
        self.state
            .add_user_message("Please start the task. Think step-by-step.".to_string());

        let max_turns = 10;
        for _ in 0..max_turns {
            let model = "best-available";

            let response = inference::chat(model, &self.state.history)
                .map_err(|e| anyhow!("Inference error: {:?}", e))?;

            let response_content = response.content;
            self.state.add_assistant_message(response_content.clone());

            let execution_result = self.tools.execute_all(&response_content)?;

            if execution_result.is_done {
                return Ok(execution_result
                    .final_output
                    .unwrap_or_else(|| "Task completed.".to_string()));
            }

            if execution_result.output.trim().is_empty() {
                self.state.add_user_message("I didn't see any tool calls. Please use tools to make progress or <done> if finished.".to_string());
            } else {
                self.state
                    .add_user_message(format!("Tool Results:\n{}", execution_result.output));
            }
        }

        Ok("Agent reached maximum turn limit.".to_string())
    }
}
