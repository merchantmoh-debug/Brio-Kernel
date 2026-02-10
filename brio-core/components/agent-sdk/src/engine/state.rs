//! Agent state management.

use crate::types::{Message, Role};

/// Internal state for the agent.
pub(crate) struct AgentState {
    pub(crate) history: Vec<Message>,
    pub(crate) iteration: u32,
}

impl AgentState {
    pub(crate) fn new(history: Vec<Message>) -> Self {
        Self {
            history,
            iteration: 0,
        }
    }

    pub(crate) fn add_message(&mut self, role: Role, content: String) {
        self.history.push(Message::new(role, content));
    }
}
