use chrono::NaiveDateTime;
use rusqlite::types::{FromSql, FromSqlError, FromSqlResult, ToSqlOutput, ValueRef};
use rusqlite::ToSql;
use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};
use std::str::FromStr;

fn parse_err(msg: String) -> FromSqlError {
    FromSqlError::Other(Box::new(std::io::Error::new(
        std::io::ErrorKind::InvalidData,
        msg,
    )))
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EventType {
    TaskCreated,
    TaskReady,
    TaskClaimed,
    TaskStarted,
    TaskCompleted,
    TaskFailed,
    TaskRetrying,
    TaskCancelled,
    DependencyAdded,
    ArtifactCreated,
    ApprovalRequested,
    ApprovalResolved,
}

impl Display for EventType {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::TaskCreated => "task_created",
            Self::TaskReady => "task_ready",
            Self::TaskClaimed => "task_claimed",
            Self::TaskStarted => "task_started",
            Self::TaskCompleted => "task_completed",
            Self::TaskFailed => "task_failed",
            Self::TaskRetrying => "task_retrying",
            Self::TaskCancelled => "task_cancelled",
            Self::DependencyAdded => "dependency_added",
            Self::ArtifactCreated => "artifact_created",
            Self::ApprovalRequested => "approval_requested",
            Self::ApprovalResolved => "approval_resolved",
        })
    }
}

impl FromStr for EventType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "task_created" => Ok(Self::TaskCreated),
            "task_ready" => Ok(Self::TaskReady),
            "task_claimed" => Ok(Self::TaskClaimed),
            "task_started" => Ok(Self::TaskStarted),
            "task_completed" => Ok(Self::TaskCompleted),
            "task_failed" => Ok(Self::TaskFailed),
            "task_retrying" => Ok(Self::TaskRetrying),
            "task_cancelled" => Ok(Self::TaskCancelled),
            "dependency_added" => Ok(Self::DependencyAdded),
            "artifact_created" => Ok(Self::ArtifactCreated),
            "approval_requested" => Ok(Self::ApprovalRequested),
            "approval_resolved" => Ok(Self::ApprovalResolved),
            _ => Err(format!("invalid event type: {s}")),
        }
    }
}

impl ToSql for EventType {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        Ok(ToSqlOutput::from(self.to_string()))
    }
}

impl FromSql for EventType {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        let text = value.as_str()?;
        Self::from_str(text).map_err(parse_err)
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Event {
    pub id: i64,
    pub task_id: Option<String>,
    pub project_id: Option<String>,
    pub agent_id: Option<String>,
    pub event_type: EventType,
    pub payload: Option<serde_json::Value>,
    pub timestamp: NaiveDateTime,
}
