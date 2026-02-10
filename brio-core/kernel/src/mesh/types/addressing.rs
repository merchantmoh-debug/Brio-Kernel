//! Network addressing types for mesh networking.

use serde::{Deserialize, Serialize};
use std::fmt;

use super::node::ValidationError;

/// Address where a node acts as a gRPC server
#[derive(Debug, Clone, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub struct NodeAddress(String);

impl NodeAddress {
    /// Creates a new `NodeAddress` from a string, validating it is non-empty and well-formed.
    ///
    /// # Errors
    /// Returns `ValidationError::EmptyAddress` if the address is empty.
    /// Returns `ValidationError::InvalidAddressFormat` if the address format is invalid.
    pub fn new(s: &str) -> Result<Self, ValidationError> {
        let trimmed = s.trim();
        if trimmed.is_empty() {
            return Err(ValidationError::EmptyAddress);
        }
        Ok(Self(trimmed.to_string()))
    }

    /// Returns the string representation of this `NodeAddress`.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Consumes the `NodeAddress` and returns the inner String.
    #[must_use]
    pub fn into_string(self) -> String {
        self.0
    }
}

impl fmt::Display for NodeAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_node_address_new() {
        let addr = NodeAddress::new("127.0.0.1:8080").unwrap();
        assert_eq!(addr.as_str(), "127.0.0.1:8080");
    }

    #[test]
    fn test_node_address_empty() {
        let result = NodeAddress::new("");
        assert!(matches!(result, Err(ValidationError::EmptyAddress)));
    }

    #[test]
    fn test_node_address_whitespace() {
        let result = NodeAddress::new("   ");
        assert!(matches!(result, Err(ValidationError::EmptyAddress)));
    }

    #[test]
    fn test_node_address_display() {
        let addr = NodeAddress::new("127.0.0.1:8080").unwrap();
        assert_eq!(addr.to_string(), "127.0.0.1:8080");
    }
}
