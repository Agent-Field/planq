use super::routes::{parse_event_stream_query, ApiError};
use crate::db::Database;
use axum::extract::{Query, State};
use axum::response::sse::{Event, KeepAlive, Sse};
use chrono::NaiveDateTime;
use futures_core::Stream;
use rusqlite::params;
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::VecDeque;
use std::convert::Infallible;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::Duration;

#[derive(Debug, Deserialize, Clone)]
pub struct EventStreamQuery {
    pub project_id: Option<String>,
    #[serde(rename = "type")]
    pub event_type: Option<String>,
}

#[derive(Debug)]
struct EventRow {
    id: i64,
    task_id: Option<String>,
    project_id: Option<String>,
    agent_id: Option<String>,
    event_type: String,
    payload: Option<Value>,
    timestamp: NaiveDateTime,
}

pub(crate) struct PollingEventStream {
    db: Arc<Database>,
    project_id: Option<String>,
    event_type: Option<String>,
    last_seen_id: i64,
    interval: tokio::time::Interval,
    buffered: VecDeque<EventRow>,
}

impl PollingEventStream {
    fn new(db: Arc<Database>, project_id: Option<String>, event_type: Option<String>) -> Self {
        Self {
            db,
            project_id,
            event_type,
            last_seen_id: 0,
            interval: tokio::time::interval(Duration::from_millis(500)),
            buffered: VecDeque::new(),
        }
    }
}

impl Stream for PollingEventStream {
    type Item = Result<Event, Infallible>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();

        if let Some(event) = this.buffered.pop_front() {
            this.last_seen_id = event.id;
            let payload = json!({
                "id": event.id,
                "event": event.event_type,
                "task_id": event.task_id,
                "project_id": event.project_id,
                "agent_id": event.agent_id,
                "payload": event.payload,
                "timestamp": event.timestamp,
            });
            return Poll::Ready(Some(Ok(Event::default().data(payload.to_string()))));
        }

        match Pin::new(&mut this.interval).poll_tick(cx) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(_) => {
                if let Ok(events) = fetch_events_since(
                    &this.db,
                    this.last_seen_id,
                    this.project_id.clone(),
                    this.event_type.clone(),
                ) {
                    this.buffered = VecDeque::from(events);
                }

                if let Some(event) = this.buffered.pop_front() {
                    this.last_seen_id = event.id;
                    let payload = json!({
                        "id": event.id,
                        "event": event.event_type,
                        "task_id": event.task_id,
                        "project_id": event.project_id,
                        "agent_id": event.agent_id,
                        "payload": event.payload,
                        "timestamp": event.timestamp,
                    });
                    Poll::Ready(Some(Ok(Event::default().data(payload.to_string()))))
                } else {
                    Poll::Pending
                }
            }
        }
    }
}

pub(crate) async fn event_stream_handler(
    State(db): State<Arc<Database>>,
    Query(query): Query<EventStreamQuery>,
) -> Result<Sse<PollingEventStream>, ApiError> {
    let (project_id, event_type) = parse_event_stream_query(&query)?;
    let stream = PollingEventStream::new(db, project_id, event_type.map(|v| v.to_string()));
    Ok(Sse::new(stream).keep_alive(
        KeepAlive::new()
            .interval(Duration::from_secs(15))
            .text("keep-alive"),
    ))
}

fn fetch_events_since(
    db: &Database,
    last_seen_id: i64,
    project_id: Option<String>,
    event_type: Option<String>,
) -> anyhow::Result<Vec<EventRow>> {
    let conn = db.lock()?;
    let mut stmt = conn.prepare(
        r#"
        SELECT id, task_id, project_id, agent_id, event_type, payload, timestamp
        FROM events
        WHERE id > ?1
          AND (?2 IS NULL OR project_id = ?2)
          AND (?3 IS NULL OR event_type = ?3)
        ORDER BY id ASC
        "#,
    )?;

    let mut rows = stmt.query(params![last_seen_id, project_id, event_type])?;
    let mut out = Vec::new();
    while let Some(row) = rows.next()? {
        let payload_raw: Option<String> = row.get(5)?;
        let timestamp_raw: String = row.get(6)?;
        let timestamp = NaiveDateTime::parse_from_str(&timestamp_raw, "%Y-%m-%d %H:%M:%S")?;
        out.push(EventRow {
            id: row.get(0)?,
            task_id: row.get(1)?,
            project_id: row.get(2)?,
            agent_id: row.get(3)?,
            event_type: row.get(4)?,
            payload: payload_raw.and_then(|v| serde_json::from_str::<Value>(&v).ok()),
            timestamp,
        });
    }
    Ok(out)
}
