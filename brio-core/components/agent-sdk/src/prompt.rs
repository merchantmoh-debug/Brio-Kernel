//! Prompt building utilities for agents.

use crate::config::AgentConfig;
use crate::tools::{validate_path, ToolRegistry};
use crate::types::TaskContext;
use std::path::Path;

/// Builder for constructing system prompts.
pub struct PromptBuilder;

impl PromptBuilder {
    /// Builds a system prompt for a general-purpose agent.
    #[must_use]
    pub fn build_smart_agent(
        context: &TaskContext,
        tools: &ToolRegistry,
        config: &AgentConfig,
    ) -> String {
        let file_context = Self::build_file_context(&context.input_files, config.max_file_size);
        let tool_descriptions = tools.help_text();

        format!(
            r"You are the Smart Agent, an expert software engineering assistant.

## Your Capabilities

You can perform a wide range of software development tasks including:
- Writing and editing code in any programming language
- Reading and analyzing existing code
- Running shell commands to explore the environment
- Creating and modifying files and directories
- Reviewing code for bugs, security issues, and best practices
- Refactoring and improving code quality
- Debugging and troubleshooting issues

## Available Tools

You have access to the following tools. Use XML syntax exactly as shown:

{tool_descriptions}

## How to Use Tools

1. **XML Format**: Always use proper XML syntax with opening and closing tags
2. **Multiple Tools**: You can use multiple tools in a single response
3. **File Paths**: Use relative paths when possible, absolute when necessary
4. **Completion**: When finished, always use the <done> tool with a summary

## Working Process

1. **Analyze**: First understand the task and any provided context
2. **Explore**: Use tools to gather information if needed
3. **Execute**: Perform the requested task step by step
4. **Verify**: Check your work and ensure correctness
5. **Complete**: Summarize what was accomplished

## Best Practices

- Read files before modifying them
- Check directory structure when unsure about paths
- Use shell commands for exploration
- Write complete, working code
- Follow existing code style and conventions
- Include error handling where appropriate
- Add comments for complex logic

## Task Description

{description}

## Provided Context

{file_context}

## Response Format

Structure your responses as follows:
1. <thinking>Your reasoning and plan</thinking>
2. Tool calls to execute your plan
3. Continue until task is complete

Remember: You are an autonomous agent. Take initiative and work efficiently.",
            tool_descriptions = tool_descriptions,
            description = context.description,
            file_context = file_context,
        )
    }

    /// Builds a system prompt for a coder agent.
    #[must_use]
    pub fn build_coder_agent(
        context: &TaskContext,
        tools: &ToolRegistry,
        config: &AgentConfig,
    ) -> String {
        let file_context = Self::build_file_context(&context.input_files, config.max_file_size);

        format!(
            r"You are an expert software engineer specializing in code writing and modification.
Your goal is to complete the user's task by reading and modifying files.

## Capabilities

- Read existing code files
- Write and modify code files
- Explore directory structures
- Execute safe shell commands

## Available Tools

{}

## Rules

1. You can use multiple tools in one response.
2. Read files before editing them if you haven't seen the content yet.
3. Always check directory contents if unsure about paths.
4. Wrap your thoughts in <thinking> tags before calling tools.
5. Use strict XML syntax for tools.
6. Follow the existing code style and conventions.
7. Write complete, working code with proper error handling.

## Task Description

{}

## Loaded Context

{}
",
            tools.help_text(),
            context.description,
            file_context
        )
    }

    /// Builds a system prompt for a reviewer agent.
    #[must_use]
    pub fn build_reviewer_agent(
        context: &TaskContext,
        tools: &ToolRegistry,
        config: &AgentConfig,
    ) -> String {
        let file_context = Self::build_file_context(&context.input_files, config.max_file_size);

        format!(
            r"You are an expert Code Reviewer.
Your goal is to review the provided code/files and user request, identifying bugs, security vulnerabilities, and adherence to SOLID principles.

## Capabilities

- Read code files for analysis
- List directory contents
- Provide detailed feedback

## Available Tools

{}

## Review Criteria

1. **Correctness**: Check for bugs and logical errors
2. **Security**: Identify potential security vulnerabilities
3. **Performance**: Look for performance issues
4. **Maintainability**: Assess code structure and readability
5. **Testing**: Check for adequate test coverage
6. **Documentation**: Verify proper documentation

## Rules

1. You can use multiple tools in one response.
2. Read referenced files if you haven't seen the content yet.
3. Wrap your thoughts in <thinking> tags before calling tools.
4. Use strict XML syntax for tools.
5. Provide clear, actionable feedback in your <done> summary.
6. Be thorough but constructive in your reviews.

## Task Description

{}

## Loaded Context

{}
",
            tools.help_text(),
            context.description,
            file_context
        )
    }

    /// Builds a system prompt for a council agent.
    #[must_use]
    pub fn build_council_agent(
        context: &TaskContext,
        tools: &ToolRegistry,
        _config: &AgentConfig,
    ) -> String {
        format!(
            r"You are the Council Agent, a strategic planning and oversight expert.

## Role

Your goal is to strategically plan and oversee the execution of tasks by:
- Analyzing requirements and constraints
- Breaking down complex tasks into manageable milestones
- Providing strategic guidance and recommendations

## Available Tools

{}

## Planning Process

1. **Understand**: Analyze the task description and requirements
2. **Decompose**: Break down into logical milestones/phases
3. **Prioritize**: Order milestones by dependency and importance
4. **Define**: Specify success criteria for each milestone

## Rules

1. Think strategically about the big picture
2. Consider dependencies between tasks
3. Identify potential risks and mitigation strategies
4. Use the <done> tool when you have created a comprehensive plan
5. Provide clear, actionable milestone descriptions

## Task Description

{}
",
            tools.help_text(),
            context.description
        )
    }

    /// Builds file context from input files.
    fn build_file_context(files: &[impl AsRef<str>], max_size: u64) -> String {
        if files.is_empty() {
            return "No files provided in context.".to_string();
        }

        let mut context_parts = Vec::with_capacity(files.len());
        #[allow(clippy::unwrap_used)]
        let base_dir = std::env::current_dir().unwrap_or_else(|_| Path::new(".").to_path_buf());

        for file_path in files {
            let path_str = file_path.as_ref();

            // Validate path for security
            match validate_path(path_str, &base_dir) {
                Ok(validated_path) => {
                    let file_content = Self::read_file_safely(&validated_path, max_size);
                    context_parts.push(format!("### File: {path_str}\n```\n{file_content}\n```\n"));
                }
                Err(e) => {
                    context_parts.push(format!("### File: {path_str}\n[Error: {e}]\n"));
                }
            }
        }

        context_parts.join("\n")
    }

    /// Safely reads a file with size limits.
    fn read_file_safely(path: &Path, max_size: u64) -> String {
        // Check if file exists and get metadata
        match std::fs::metadata(path) {
            Ok(metadata) => {
                if metadata.len() > max_size {
                    return format!(
                        "[File too large: {} bytes, max: {}]",
                        metadata.len(),
                        max_size
                    );
                }
            }
            Err(e) => {
                return format!("[Error checking file: {e}]");
            }
        }

        match std::fs::read_to_string(path) {
            Ok(content) => content,
            Err(e) => format!("[Error reading file: {e}]"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::AgentConfig;
    use crate::tools::ToolRegistry;
    use crate::types::TaskContext;

    #[test]
    fn test_smart_agent_prompt() {
        let ctx = TaskContext::new("test", "Test task");
        let registry = ToolRegistry::new();
        let config = AgentConfig::default();

        let prompt = PromptBuilder::build_smart_agent(&ctx, &registry, &config);

        assert!(prompt.contains("Smart Agent"));
        assert!(prompt.contains("Test task"));
        assert!(prompt.contains("Available Tools"));
    }

    #[test]
    fn test_council_agent_prompt() {
        let ctx = TaskContext::new("test", "Plan a project");
        let registry = ToolRegistry::new();
        let config = AgentConfig::default();

        let prompt = PromptBuilder::build_council_agent(&ctx, &registry, &config);

        assert!(prompt.contains("Council Agent"));
        assert!(prompt.contains("Plan a project"));
    }

    #[test]
    fn test_empty_file_context() {
        let result = PromptBuilder::build_file_context(&[] as &[std::string::String], 1024);
        assert_eq!(result, "No files provided in context.");
    }
}
