//! Tool system with Type-State pattern and security validation.

pub mod constants;
pub mod parser;
pub mod registry;
pub mod validation;
pub mod wasm_bridge;

use crate::error::ToolError;
use std::borrow::Cow;
use std::collections::HashMap;

/// Trait for tools that can be executed by agents.
pub trait Tool: Send + Sync {
    /// Returns the unique name of the tool.
    fn name(&self) -> Cow<'static, str>;

    /// Returns the description of the tool in XML format.
    fn description(&self) -> Cow<'static, str>;

    /// Executes the tool with the provided arguments.
    ///
    /// # Errors
    ///
    /// Returns an error if the tool execution fails or the arguments are invalid.
    fn execute(&self, args: &HashMap<String, String>) -> Result<String, ToolError>;
}

pub use parser::{ArgExtractor, ToolParser};
pub use registry::ToolRegistry;
pub use validation::{
    SecureFilePath, Unvalidated, Validated, validate_file_size, validate_path,
    validate_shell_command,
};
