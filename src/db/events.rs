use crate::db::{dt_to_sql, json_to_sql, parse_dt, parse_json, Database};
use crate::models::{Event, EventType};
use anyhow::Result;
use chrono::NaiveDateTime;
use rusqlite::params;

const INSERT_EVENT: &str = r#"
INSERT INTO events(task_id, project_id, agent_id, event_type, payload, timestamp)
VALUES (?1, ?2, ?3, ?4, ?5, ?6);
"#;

const SELECT_EVENT_BY_ID: &str = r#"
SELECT id, task_id, project_id, agent_id, event_type, payload, timestamp
FROM events
WHERE id = ?1;
"#;

const LIST_EVENTS_FILTERED: &str = r#"
SELECT id, task_id, project_id, agent_id, event_type, payload, timestamp
FROM events
WHERE (?1 IS NULL OR project_id = ?1)
  AND (?2 IS NULL OR task_id = ?2)
  AND (?3 IS NULL OR event_type = ?3)
  AND (?4 IS NULL OR timestamp >= ?4)
ORDER BY timestamp ASC, id ASC;
"#;

#[derive(Default, Clone, Debug)]
pub struct EventFilters {
    pub project_id: Option<String>,
    pub task_id: Option<String>,
    pub event_type: Option<EventType>,
    pub since: Option<NaiveDateTime>,
}

fn row_to_event(row: &rusqlite::Row<'_>) -> rusqlite::Result<Event> {
    let conv = |idx: usize, e: anyhow::Error| {
        rusqlite::Error::FromSqlConversionFailure(
            idx,
            rusqlite::types::Type::Text,
            Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                e.to_string(),
            )),
        )
    };
    let payload: Option<String> = row.get(5)?;
    Ok(Event {
        id: row.get(0)?,
        task_id: row.get(1)?,
        project_id: row.get(2)?,
        agent_id: row.get(3)?,
        event_type: row.get(4)?,
        payload: parse_json(payload).map_err(|e| conv(5, e))?,
        timestamp: parse_dt(row.get::<_, String>(6)?).map_err(|e| conv(6, e))?,
    })
}

pub fn insert_event(
    db: &Database,
    task_id: Option<&str>,
    project_id: Option<&str>,
    agent_id: Option<&str>,
    event_type: EventType,
    payload: Option<serde_json::Value>,
    timestamp: NaiveDateTime,
) -> Result<Event> {
    let conn = db.lock()?;
    conn.execute(
        INSERT_EVENT,
        params![
            task_id,
            project_id,
            agent_id,
            event_type,
            json_to_sql(&payload)?,
            dt_to_sql(timestamp)
        ],
    )?;
    let id = conn.last_insert_rowid();
    let mut stmt = conn.prepare(SELECT_EVENT_BY_ID)?;
    let event = stmt.query_row(params![id], row_to_event)?;
    Ok(event)
}

pub fn list_events(db: &Database, filters: EventFilters) -> Result<Vec<Event>> {
    let conn = db.lock()?;
    let mut stmt = conn.prepare(LIST_EVENTS_FILTERED)?;
    let mut rows = stmt.query(params![
        filters.project_id,
        filters.task_id,
        filters.event_type.map(|e| e.to_string()),
        filters.since.map(dt_to_sql),
    ])?;
    let mut out = Vec::new();
    while let Some(row) = rows.next()? {
        out.push(row_to_event(row)?);
    }
    Ok(out)
}
