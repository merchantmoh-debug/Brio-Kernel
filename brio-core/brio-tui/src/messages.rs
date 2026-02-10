use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientMessage {
    Task {
        content: String,
    },
    Session {
        action: SessionAction,
        #[serde(flatten)]
        params: SessionParams,
    },
    Query {
        sql: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionAction {
    Begin,
    Commit,
    Rollback,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SessionParams {
    pub base_path: Option<String>,
    pub session_id: Option<String>,
}
