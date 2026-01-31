use crate::tools::ToolRegistry;
use std::fs;

pub struct PromptBuilder;

impl PromptBuilder {
    pub fn build(
        context: &crate::exports::brio::core::agent_runner::TaskContext,
        tools: &ToolRegistry,
    ) -> String {
        let mut file_contexts = String::new();
        for file_path in &context.input_files {
            match fs::read_to_string(file_path) {
                Ok(content) => {
                    file_contexts
                        .push_str(&format!("\n--- File: {} ---\n{}\n", file_path, content));
                }
                Err(e) => {
                    file_contexts.push_str(&format!(
                        "\n--- File: {} (Error reading: {}) ---\n",
                        file_path, e
                    ));
                }
            }
        }

        format!(
            r#"You are an expert Code Reviewer.
Your goal is to review the provided code/files and user request, identifying bugs, security vulnerabilities, and adherence to SOLID principles.

Available Tools (XML syntax):
{}

Rules:
1. You can use multiple tools in one response.
2. Read referenced files if you haven't seen the content yet.
3. Wrap your thoughts in <thinking> tags before calling tools.
4. Strict XML syntax for tools.
5. Provide clear, actionable feedback in your <done> summary.

Task Description:
{}

Loaded Context:
{}
"#,
            tools.help_text(),
            context.description,
            file_contexts
        )
    }
}
