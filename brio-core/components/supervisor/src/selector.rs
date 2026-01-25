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
        let content = task.content().to_lowercase();
        if content.contains("review") || content.contains("audit") || content.contains("check") {
            AgentId::new("agent_reviewer")
        } else {
            AgentId::new("agent_coder")
        }
    }
}
