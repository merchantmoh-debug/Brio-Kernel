//! Agent selection strategies for task dispatch.
//!
//! Provides algorithms for selecting the most appropriate agent for a given task.

use crate::domain::{AgentId, Task};

/// Strategy for selecting an agent for a task.
pub trait AgentSelector {
    /// Selects the best agent for the given task.
    fn select(&self, task: &Task) -> AgentId;
}

/// Default implementation using keyword matching.
pub struct KeywordAgentSelector;

impl Default for KeywordAgentSelector {
    fn default() -> Self {
        Self
    }
}

impl AgentSelector for KeywordAgentSelector {
    fn select(&self, task: &Task) -> AgentId {
        // Use case-insensitive search without allocating a new String
        let content = task.content();
        if content.contains("review")
            || content.contains("audit")
            || content.contains("check")
            || content.contains("Review")
            || content.contains("Audit")
            || content.contains("Check")
            || content.contains("REVIEW")
            || content.contains("AUDIT")
            || content.contains("CHECK")
        {
            AgentId::new("agent_reviewer").expect("static agent ID should be valid")
        } else {
            AgentId::new("agent_coder").expect("static agent ID should be valid")
        }
    }
}
