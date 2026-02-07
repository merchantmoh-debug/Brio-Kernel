//! Event bus for pub/sub messaging between plugins.

use parking_lot::RwLock;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

/// Event bus for managing topic subscriptions.
#[derive(Clone, Default)]
pub struct EventBus {
    /// Map of topic names to sets of subscribed plugin IDs
    subscriptions: Arc<RwLock<HashMap<String, HashSet<String>>>>,
}

impl EventBus {
    /// Creates a new event bus with no subscriptions
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Subscribes a plugin to a topic.
    ///
    /// # Arguments
    ///
    /// * `topic` - The topic to subscribe to.
    /// * `plugin_id` - The ID of the plugin subscribing.
    pub fn subscribe(&self, topic: String, plugin_id: String) {
        let mut subs = self.subscriptions.write();
        subs.entry(topic).or_default().insert(plugin_id);
    }

    /// Returns the list of subscribers for a topic.
    ///
    /// # Arguments
    ///
    /// * `topic` - The topic to get subscribers for.
    ///
    /// # Returns
    ///
    /// A vector of plugin IDs subscribed to the topic.
    #[must_use]
    pub fn subscribers(&self, topic: &str) -> Vec<String> {
        let subs = self.subscriptions.read();
        subs.get(topic)
            .map(|s| s.iter().cloned().collect())
            .unwrap_or_default()
    }
}
