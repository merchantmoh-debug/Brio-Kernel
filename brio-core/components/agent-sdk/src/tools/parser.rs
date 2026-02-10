//! Tool parser for extracting tool invocations.

use crate::types::ToolInvocation;
use regex::{Captures, Regex};
use std::collections::HashMap;

/// Type alias for tool argument extractor functions.
pub type ArgExtractor = Box<dyn Fn(&Captures) -> HashMap<String, String> + Send + Sync>;

/// Parser for extracting tool invocations from agent responses.
pub struct ToolParser {
    /// Compiled regex pattern.
    regex: Regex,
    /// Function to extract arguments from captures.
    extractor: ArgExtractor,
}

impl ToolParser {
    /// Creates a new tool parser.
    ///
    /// # Errors
    ///
    /// Returns an error if the regex pattern is invalid.
    pub fn new<E>(pattern: &str, extractor: E) -> Result<Self, regex::Error>
    where
        E: Fn(&Captures) -> HashMap<String, String> + Send + Sync + 'static,
    {
        let regex = Regex::new(pattern)?;
        Ok(Self {
            regex,
            extractor: Box::new(extractor),
        })
    }

    /// Creates a new tool parser from a known-valid regex pattern.
    ///
    /// # Panics
    ///
    /// Panics if the regex pattern is invalid. This should only be used
    /// with compile-time validated patterns in static initializers.
    #[inline]
    pub fn new_unchecked<E>(pattern: &str, extractor: E) -> Self
    where
        E: Fn(&Captures) -> HashMap<String, String> + Send + Sync + 'static,
    {
        match Self::new(pattern, extractor) {
            Ok(parser) => parser,
            Err(e) => panic!("regex pattern should be valid at compile time: {e}"),
        }
    }

    /// Creates a new tool parser from a pre-compiled regex.
    ///
    /// This is useful when you want to reuse a compiled regex pattern
    /// that was created elsewhere (e.g., in a static `OnceLock`).
    #[must_use]
    pub fn from_regex<E>(regex: &Regex, extractor: E) -> Self
    where
        E: Fn(&Captures) -> HashMap<String, String> + Send + Sync + 'static,
    {
        Self {
            regex: regex.clone(),
            extractor: Box::new(extractor),
        }
    }

    /// Parses tool invocations from the input text.
    #[must_use]
    pub fn parse(&self, input: &str) -> Vec<ToolInvocation> {
        let mut results = Vec::new();

        for mat in self.regex.find_iter(input) {
            if let Some(caps) = self.regex.captures(mat.as_str()) {
                let args = (self.extractor)(&caps);

                results.push(ToolInvocation {
                    name: Self::extract_tool_name(&caps),
                    args,
                    position: mat.start(),
                });
            }
        }

        // Sort by position to maintain order
        results.sort_by_key(|inv| inv.position);
        results
    }

    fn extract_tool_name(caps: &Captures) -> String {
        // First capture group is typically the tool name
        caps.get(1)
            .map(|m| m.as_str().to_string())
            .unwrap_or_default()
    }
}
