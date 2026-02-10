//! Message types for inference operations.
//!
//! This module contains message and role definitions used in conversations.

use serde::{Deserialize, Serialize};

/// The role of a message in a conversation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    /// System-level instructions
    System,
    /// User input
    User,
    /// Assistant response
    Assistant,
}

/// A message in a conversation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    /// The role of the message sender
    pub role: Role,
    /// The content of the message
    pub content: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_role_serialization() {
        let system = Role::System;
        let json = serde_json::to_string(&system).unwrap();
        assert_eq!(json, "\"system\"");

        let user = Role::User;
        let json = serde_json::to_string(&user).unwrap();
        assert_eq!(json, "\"user\"");

        let assistant = Role::Assistant;
        let json = serde_json::to_string(&assistant).unwrap();
        assert_eq!(json, "\"assistant\"");
    }

    #[test]
    fn test_role_deserialization() {
        let system: Role = serde_json::from_str("\"system\"").unwrap();
        assert!(matches!(system, Role::System));

        let user: Role = serde_json::from_str("\"user\"").unwrap();
        assert!(matches!(user, Role::User));

        let assistant: Role = serde_json::from_str("\"assistant\"").unwrap();
        assert!(matches!(assistant, Role::Assistant));
    }

    #[test]
    fn test_message_creation() {
        let msg = Message {
            role: Role::User,
            content: "Hello".to_string(),
        };
        assert!(matches!(msg.role, Role::User));
        assert_eq!(msg.content, "Hello");
    }

    #[test]
    fn test_message_serialization() {
        let msg = Message {
            role: Role::Assistant,
            content: "Hi there".to_string(),
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"role\":\"assistant\""));
        assert!(json.contains("\"content\":\"Hi there\""));
    }
}
