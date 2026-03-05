use crate::db::tasks::promote_ready_tasks;
use crate::db::{dt_to_sql, Database};
use crate::models::{EventType, RetryBackoff};
use anyhow::Result;
use chrono::Duration;
use rusqlite::params;

const SELECT_EXPIRED_HEARTBEAT_RUNNING: &str = r#"
SELECT id, project_id, last_heartbeat, heartbeat_interval
FROM tasks
WHERE status = 'running'
  AND last_heartbeat IS NOT NULL;
"#;

const RECLAIM_TASK: &str = r#"
UPDATE tasks
SET status = 'ready', agent_id = NULL, claimed_at = NULL, started_at = NULL, updated_at = ?2
WHERE id = ?1;
"#;

const SELECT_TIMED_OUT_TASKS: &str = r#"
SELECT id, project_id, started_at, timeout_seconds
FROM tasks
WHERE status = 'running'
  AND timeout_seconds IS NOT NULL
  AND started_at IS NOT NULL;
"#;

const FAIL_TIMEOUT_TASK: &str = r#"
UPDATE tasks
SET status = 'failed', error = 'timeout', completed_at = ?2, updated_at = ?2
WHERE id = ?1;
"#;

const SELECT_RETRYABLE_FAILED: &str = r#"
SELECT id, project_id, retry_count, max_retries, retry_backoff, retry_delay_ms, completed_at
FROM tasks
WHERE status = 'failed'
  AND retry_count < max_retries
  AND completed_at IS NOT NULL;
"#;

const RETRY_TASK: &str = r#"
UPDATE tasks
SET status = 'ready', retry_count = retry_count + 1, error = NULL, completed_at = NULL, updated_at = ?2
WHERE id = ?1;
"#;

const SELECT_COMPOSITES_ALL_CHILDREN_DONE: &str = r#"
SELECT p.id, p.project_id
FROM tasks p
WHERE p.is_composite = 1
  AND p.status IN ('pending', 'ready', 'claimed', 'running')
  AND EXISTS (SELECT 1 FROM tasks c WHERE c.parent_task_id = p.id)
  AND NOT EXISTS (
    SELECT 1 FROM tasks c
    WHERE c.parent_task_id = p.id
      AND c.status NOT IN ('done', 'done_partial')
  );
"#;

const COMPLETE_COMPOSITE: &str = r#"
UPDATE tasks
SET status = 'done', completed_at = ?2, updated_at = ?2
WHERE id = ?1;
"#;

const SELECT_COMPOSITES_ANY_CHILD_PERMA_FAILED: &str = r#"
SELECT DISTINCT p.id, p.project_id
FROM tasks p
JOIN tasks c ON c.parent_task_id = p.id
WHERE p.is_composite = 1
  AND p.status IN ('pending', 'ready', 'claimed', 'running')
  AND c.status = 'failed'
  AND c.retry_count >= c.max_retries;
"#;

const FAIL_COMPOSITE: &str = r#"
UPDATE tasks
SET status = 'failed', error = 'child task permanently failed', completed_at = ?2, updated_at = ?2
WHERE id = ?1;
"#;

const INSERT_EVENT: &str = r#"
INSERT INTO events(task_id, project_id, agent_id, event_type, payload, timestamp)
VALUES (?1, ?2, NULL, ?3, ?4, ?5);
"#;

#[derive(Default, Debug, Clone, Copy)]
pub struct SweepResult {
    pub promoted: usize,
    pub reclaimed: usize,
    pub timed_out: usize,
    pub retried: usize,
    pub composites_completed: usize,
}

fn retry_delay_ms(base_delay: i64, backoff: RetryBackoff, retry_count: i32) -> i64 {
    match backoff {
        RetryBackoff::Fixed => base_delay,
        RetryBackoff::Linear => base_delay * i64::from(retry_count + 1),
        RetryBackoff::Exponential => {
            let power = (retry_count + 1).max(0) as u32;
            base_delay.saturating_mul(2_i64.saturating_pow(power))
        }
    }
}

pub fn run_sweep(db: &Database) -> Result<SweepResult> {
    let now = chrono::Utc::now().naive_utc();
    let now_s = dt_to_sql(now);
    let mut result = SweepResult::default();

    {
        let conn = db.lock()?;
        let mut stmt = conn.prepare(SELECT_EXPIRED_HEARTBEAT_RUNNING)?;
        let mut rows = stmt.query([])?;
        let mut targets: Vec<(String, String)> = Vec::new();
        while let Some(row) = rows.next()? {
            let last_heartbeat_s: String = row.get(2)?;
            let heartbeat_interval: i64 = row.get::<_, i32>(3)? as i64;
            let last_heartbeat =
                chrono::NaiveDateTime::parse_from_str(&last_heartbeat_s, "%Y-%m-%d %H:%M:%S")?;
            if now - last_heartbeat >= Duration::seconds(heartbeat_interval * 3) {
                targets.push((row.get(0)?, row.get(1)?));
            }
        }
        drop(rows);
        drop(stmt);
        for (task_id, project_id) in targets {
            let changed = conn.execute(RECLAIM_TASK, params![task_id, now_s.clone()])?;
            if changed > 0 {
                result.reclaimed += changed;
                conn.execute(
                    INSERT_EVENT,
                    params![
                        task_id,
                        project_id,
                        EventType::TaskRetrying,
                        serde_json::to_string(&serde_json::json!({"reason": "heartbeat_timeout"}))?,
                        now_s.clone()
                    ],
                )?;
            }
        }
    }

    {
        let conn = db.lock()?;
        let mut stmt = conn.prepare(SELECT_TIMED_OUT_TASKS)?;
        let mut rows = stmt.query([])?;
        let mut targets: Vec<(String, String)> = Vec::new();
        while let Some(row) = rows.next()? {
            let started_at_s: String = row.get(2)?;
            let timeout_seconds: i64 = row.get(3)?;
            let started_at =
                chrono::NaiveDateTime::parse_from_str(&started_at_s, "%Y-%m-%d %H:%M:%S")?;
            if now - started_at >= Duration::seconds(timeout_seconds) {
                targets.push((row.get(0)?, row.get(1)?));
            }
        }
        drop(rows);
        drop(stmt);
        for (task_id, project_id) in targets {
            let changed = conn.execute(FAIL_TIMEOUT_TASK, params![task_id, now_s.clone()])?;
            if changed > 0 {
                result.timed_out += changed;
                conn.execute(
                    INSERT_EVENT,
                    params![
                        task_id,
                        project_id,
                        EventType::TaskFailed,
                        serde_json::to_string(&serde_json::json!({"reason": "timeout"}))?,
                        now_s.clone()
                    ],
                )?;
            }
        }
    }

    {
        let conn = db.lock()?;
        let mut stmt = conn.prepare(SELECT_RETRYABLE_FAILED)?;
        let mut rows = stmt.query([])?;
        let mut targets: Vec<(String, String, i32, RetryBackoff, i64, String)> = Vec::new();
        while let Some(row) = rows.next()? {
            targets.push((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(4)?,
                row.get(5)?,
                row.get(6)?,
            ));
        }
        drop(rows);
        drop(stmt);
        for (task_id, project_id, retry_count, backoff, delay_ms, completed_at_s) in targets {
            let completed_at =
                chrono::NaiveDateTime::parse_from_str(&completed_at_s, "%Y-%m-%d %H:%M:%S")?;
            let elapsed = now - completed_at;
            let required_delay = retry_delay_ms(delay_ms, backoff, retry_count);
            if elapsed < Duration::milliseconds(required_delay) {
                continue;
            }
            let changed = conn.execute(RETRY_TASK, params![task_id, now_s.clone()])?;
            if changed > 0 {
                result.retried += changed;
                conn.execute(
                    INSERT_EVENT,
                    params![
                        task_id,
                        project_id,
                        EventType::TaskRetrying,
                        serde_json::to_string(
                            &serde_json::json!({"retry_count": retry_count + 1})
                        )?,
                        now_s.clone()
                    ],
                )?;
            }
        }
    }

    {
        let conn = db.lock()?;
        let mut stmt = conn.prepare(SELECT_COMPOSITES_ALL_CHILDREN_DONE)?;
        let mut rows = stmt.query([])?;
        let mut done_targets: Vec<(String, String)> = Vec::new();
        while let Some(row) = rows.next()? {
            done_targets.push((row.get(0)?, row.get(1)?));
        }
        drop(rows);
        drop(stmt);
        for (task_id, project_id) in done_targets {
            let changed = conn.execute(COMPLETE_COMPOSITE, params![task_id, now_s.clone()])?;
            if changed > 0 {
                result.composites_completed += changed;
                conn.execute(
                    INSERT_EVENT,
                    params![
                        task_id,
                        project_id,
                        EventType::TaskCompleted,
                        serde_json::to_string(&serde_json::json!({"reason": "composite_rollup"}))?,
                        now_s.clone()
                    ],
                )?;
            }
        }

        let mut fail_stmt = conn.prepare(SELECT_COMPOSITES_ANY_CHILD_PERMA_FAILED)?;
        let mut fail_rows = fail_stmt.query([])?;
        let mut fail_targets: Vec<(String, String)> = Vec::new();
        while let Some(row) = fail_rows.next()? {
            fail_targets.push((row.get(0)?, row.get(1)?));
        }
        drop(fail_rows);
        drop(fail_stmt);
        for (task_id, project_id) in fail_targets {
            let changed = conn.execute(FAIL_COMPOSITE, params![task_id, now_s.clone()])?;
            if changed > 0 {
                conn.execute(
                    INSERT_EVENT,
                    params![
                        task_id,
                        project_id,
                        EventType::TaskFailed,
                        serde_json::to_string(
                            &serde_json::json!({"reason": "composite_child_failed"})
                        )?,
                        now_s.clone()
                    ],
                )?;
            }
        }
    }

    result.promoted = promote_ready_tasks(db)?.len();
    Ok(result)
}
