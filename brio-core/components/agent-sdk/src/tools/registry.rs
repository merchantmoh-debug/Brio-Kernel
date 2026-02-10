//! Tool registry for managing and executing tools.

use crate::error::ToolError;
use crate::tools::Tool;
use crate::tools::constants;
use crate::tools::parser::ToolParser;
use crate::types::{ExecutionResult, ToolInvocation, ToolResult};
use std::collections::HashMap;
use std::fmt::Write;
use std::sync::Arc;
use std::time::Instant;

/// Registry for managing and executing tools.
pub struct ToolRegistry {
    tools: HashMap<String, Box<dyn Tool>>,
    parsers: HashMap<String, Arc<ToolParser>>,
}

impl std::fmt::Debug for ToolRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ToolRegistry")
            .field("tools", &self.tools.keys().collect::<Vec<_>>())
            .field("parsers", &self.parsers.keys().collect::<Vec<_>>())
            .finish()
    }
}

impl ToolRegistry {
    /// Creates a new empty tool registry.
    #[must_use]
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
            parsers: HashMap::new(),
        }
    }

    /// Registers a tool with its parser.
    pub fn register(
        &mut self,
        name: impl Into<String>,
        tool: Box<dyn Tool>,
        parser: impl Into<Arc<ToolParser>>,
    ) {
        let name = name.into();
        self.tools.insert(name.clone(), tool);
        self.parsers.insert(name, parser.into());
    }

    /// Returns a list of available tool names.
    #[must_use]
    pub fn available_tools(&self) -> Vec<&str> {
        self.tools.keys().map(std::string::String::as_str).collect()
    }

    /// Returns help text for all registered tools.
    #[must_use]
    pub fn help_text(&self) -> String {
        self.tools
            .values()
            .map(|t| t.description())
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Executes all tool invocations found in the input.
    ///
    /// # Errors
    ///
    /// Returns an error if a tool is not found or if tool execution fails.
    pub fn execute_all(&self, input: &str) -> Result<ExecutionResult, ToolError> {
        let mut collected_output = String::new();
        let mut is_done = false;
        let mut final_summary = None;
        let mut tool_results = Vec::new();

        // Collect all invocations
        let mut invocations: Vec<ToolInvocation> = Vec::new();
        for (tool_name, parser) in &self.parsers {
            let mut parsed = parser.parse(input);
            for inv in &mut parsed {
                inv.name.clone_from(tool_name);
            }
            invocations.extend(parsed);
        }

        // Sort by position
        invocations.sort_by_key(|inv| inv.position);

        // Execute each invocation
        for invocation in invocations {
            if invocation.name == constants::control::DONE {
                is_done = true;
                if let Some(summary) = invocation.args.get("summary") {
                    final_summary = Some(summary.clone());
                }
                break;
            }

            match self.execute_single(&invocation) {
                Ok(result) => {
                    let _ = writeln!(
                        collected_output,
                        "✓ {}: {}",
                        invocation.name,
                        result.output.lines().next().unwrap_or(&result.output)
                    );
                    tool_results.push(result);
                }
                Err(e) => {
                    let _ = writeln!(collected_output, "✗ {} failed: {}", invocation.name, e);
                    return Err(e);
                }
            }
        }

        Ok(ExecutionResult {
            output: collected_output,
            is_complete: is_done,
            summary: final_summary,
            tool_results,
        })
    }

    /// Executes a single tool invocation.
    fn execute_single(&self, invocation: &ToolInvocation) -> Result<ToolResult, ToolError> {
        let tool = self
            .tools
            .get(&invocation.name)
            .ok_or_else(|| ToolError::NotFound {
                name: invocation.name.clone(),
            })?;

        let start = Instant::now();

        match tool.execute(&invocation.args) {
            Ok(output) => Ok(ToolResult {
                success: true,
                output,
                duration: start.elapsed(),
            }),
            Err(e) => Err(ToolError::ExecutionFailed {
                tool: invocation.name.clone(),
                source: Box::new(e),
            }),
        }
    }
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::Tool;
    use regex::Captures;
    use std::borrow::Cow;

    struct TestTool;
    impl Tool for TestTool {
        fn name(&self) -> Cow<'static, str> {
            Cow::Borrowed("test")
        }
        fn description(&self) -> Cow<'static, str> {
            Cow::Borrowed("<test />")
        }
        fn execute(&self, _args: &HashMap<String, String>) -> Result<String, ToolError> {
            Ok("test result".to_string())
        }
    }

    #[test]
    fn test_tool_registry() {
        let mut registry = ToolRegistry::new();

        let parser = Arc::new(
            ToolParser::new(r"\u003ctest\s*/?\u003e", |_caps: &Captures| {
                let mut args = HashMap::new();
                args.insert("arg".to_string(), "value".to_string());
                args
            })
            .unwrap(),
        );

        registry.register("test", Box::new(TestTool), parser);
        assert_eq!(registry.available_tools(), vec!["test"]);
    }
}
