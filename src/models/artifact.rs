use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Artifact {
    pub id: String,
    pub task_id: String,
    pub name: String,
    pub kind: Option<String>,
    pub content: Option<String>,
    pub path: Option<String>,
    pub size_bytes: Option<i64>,
    pub mime_type: Option<String>,
    pub metadata: Option<serde_json::Value>,
    pub created_at: NaiveDateTime,
}
