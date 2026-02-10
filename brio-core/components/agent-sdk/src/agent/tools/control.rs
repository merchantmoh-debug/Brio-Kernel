//! Control tools for agent task management.
//!
//! This module provides tools for controlling agent execution flow,
//! such as marking tasks as complete.

use crate::error::ToolError;
use crate::tools::Tool;
use crate::tools::constants::control;
use std::borrow::Cow;
use std::collections::HashMap;

/// Tool for marking a task as complete.
///
/// The `DoneTool` is used by agents to signal that they have finished
/// their assigned task. When executed, it returns a success message
/// indicating task completion.
///
/// # Example
///
/// ```
/// use agent_sdk::agent::tools::DoneTool;
/// use agent_sdk::Tool;
/// use std::collections::HashMap;
///
/// # fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let tool = DoneTool;
/// let args = HashMap::new();
/// let result = tool.execute(&args)?;
/// assert!(result.contains("complete"));
/// # Ok(())
/// # }
/// ```
pub struct DoneTool;

impl Tool for DoneTool {
    fn name(&self) -> Cow<'static, str> {
        Cow::Borrowed(control::DONE)
    }

    fn description(&self) -> Cow<'static, str> {
        Cow::Borrowed("<done>summary of completion</done> - Mark task as complete")
    }

    fn execute(&self, _args: &HashMap<String, String>) -> Result<String, ToolError> {
        Ok("Task marked as complete".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_done_tool_name() {
        let tool = DoneTool;
        assert_eq!(tool.name(), "done");
    }

    #[test]
    fn test_done_tool_description() {
        let tool = DoneTool;
        assert!(tool.description().contains("done"));
        assert!(tool.description().contains("complete"));
    }

    #[test]
    fn test_done_tool_execution() {
        let tool = DoneTool;
        let args = HashMap::new();
        let result = tool.execute(&args).unwrap();

        assert!(result.contains("complete"));
    }
}
