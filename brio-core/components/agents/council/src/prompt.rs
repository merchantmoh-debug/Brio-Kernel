use crate::tools::ToolRegistry;

pub struct PromptBuilder;

impl PromptBuilder {
    pub fn build(
        context: &crate::exports::brio::core::agent_runner::TaskContext,
        tools: &ToolRegistry,
    ) -> String {
        format!(
            r#"You are the Council Agent.
Your goal is to strategically plan and oversee the execution of tasks.

Available Tools (XML syntax):
{}

Task Description:
{}

Rules:
1. Analyze the task and propose milestones.
2. Use the <done> tool when you have verified the objective or completed the planning.
3. Strict XML syntax for tools.
"#,
            tools.help_text(),
            context.description,
        )
    }
}
