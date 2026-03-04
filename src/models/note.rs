use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct TaskNote {
    pub id: String,
    pub task_id: String,
    pub agent_id: Option<String>,
    pub content: String,
    pub created_at: NaiveDateTime,
}
