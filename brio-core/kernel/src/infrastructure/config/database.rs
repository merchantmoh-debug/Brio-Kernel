//! Database configuration for the Brio kernel.
//!
//! This module defines database connection settings.

use secrecy::SecretString;
use serde::Deserialize;

/// Database connection settings.
#[derive(Debug, Deserialize, Clone)]
pub struct DatabaseSettings {
    /// Database connection URL.
    pub url: SecretString,
}
