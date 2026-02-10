//! Request types for inference operations.
//!
//! This module contains request definitions for chat completions.

use crate::inference::types::message::Message;

/// Request for a chat completion.
#[derive(Debug, Clone)]
pub struct ChatRequest {
    /// The model to use for completion
    pub model: String,
    /// The conversation history
    pub messages: Vec<Message>,
}

impl ChatRequest {
    /// Creates a new chat request
    #[must_use]
    pub fn new(model: impl Into<String>, messages: Vec<Message>) -> Self {
        Self {
            model: model.into(),
            messages,
        }
    }

    /// Creates a new chat request with a single user message
    #[must_use]
    pub fn with_message(model: impl Into<String>, content: impl Into<String>) -> Self {
        use crate::inference::types::message::Role;
        Self {
            model: model.into(),
            messages: vec![Message {
                role: Role::User,
                content: content.into(),
            }],
        }
    }

    /// Adds a message to the conversation
    #[must_use]
    pub fn add_message(
        mut self,
        role: crate::inference::types::message::Role,
        content: impl Into<String>,
    ) -> Self {
        self.messages.push(Message {
            role,
            content: content.into(),
        });
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inference::types::message::{Message, Role};

    #[test]
    fn test_chat_request_new() {
        let messages = vec![Message {
            role: Role::User,
            content: "Hello".to_string(),
        }];
        let request = ChatRequest::new("gpt-4", messages);
        assert_eq!(request.model, "gpt-4");
        assert_eq!(request.messages.len(), 1);
    }

    #[test]
    fn test_chat_request_with_message() {
        let request = ChatRequest::with_message("claude-3", "Hi there");
        assert_eq!(request.model, "claude-3");
        assert_eq!(request.messages.len(), 1);
        assert!(matches!(request.messages[0].role, Role::User));
        assert_eq!(request.messages[0].content, "Hi there");
    }

    #[test]
    fn test_chat_request_add_message() {
        let request = ChatRequest::with_message("gpt-4", "Hello")
            .add_message(Role::Assistant, "Hi! How can I help?")
            .add_message(Role::User, "Tell me a joke");

        assert_eq!(request.messages.len(), 3);
        assert_eq!(request.messages[1].content, "Hi! How can I help?");
        assert_eq!(request.messages[2].content, "Tell me a joke");
    }
}
