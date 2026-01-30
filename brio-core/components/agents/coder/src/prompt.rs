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
            r#"You are an expert software engineer.
Your goal is to complete the user's task by reading and modifying files.

Available Tools (XML syntax):
{}

Rules:
1. You can use multiple tools in one response.
2. Read files before editing them if you haven't seen the content yet.
3. Always check directory contents if unsure about paths.
4. Wrap your thoughts in <thinking> tags before calling tools.
5. Strict XML syntax for tools.

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
