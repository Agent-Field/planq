use crate::db::{parse_dt, Database};
use crate::models::{generate_id, TaskNote};
use anyhow::Result;
use rusqlite::params;

const INSERT_NOTE: &str = r#"
INSERT INTO task_notes (id, task_id, agent_id, content)
VALUES (?1, ?2, ?3, ?4);
"#;

const SELECT_NOTES_BY_TASK: &str = r#"
SELECT id, task_id, agent_id, content, created_at
FROM task_notes
WHERE task_id = ?1
ORDER BY created_at ASC;
"#;

fn row_to_note(row: &rusqlite::Row<'_>) -> rusqlite::Result<TaskNote> {
    let created_at_raw: String = row.get(4)?;
    let created_at = parse_dt(created_at_raw).map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(
            4,
            rusqlite::types::Type::Text,
            Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                e.to_string(),
            )),
        )
    })?;
    Ok(TaskNote {
        id: row.get(0)?,
        task_id: row.get(1)?,
        agent_id: row.get(2)?,
        content: row.get(3)?,
        created_at,
    })
}

pub fn add_note(
    db: &Database,
    task_id: &str,
    agent_id: Option<String>,
    content: &str,
) -> Result<TaskNote> {
    let conn = db.lock()?;
    let id = generate_id("note");
    conn.execute(INSERT_NOTE, params![&id, task_id, agent_id, content])?;
    let mut stmt = conn.prepare(
        "SELECT id, task_id, agent_id, content, created_at FROM task_notes WHERE id = ?1",
    )?;
    let note = stmt.query_row(params![id], row_to_note)?;
    Ok(note)
}

pub fn list_notes(db: &Database, task_id: &str) -> Result<Vec<TaskNote>> {
    let conn = db.lock()?;
    let mut stmt = conn.prepare(SELECT_NOTES_BY_TASK)?;
    let mut rows = stmt.query(params![task_id])?;
    let mut notes = Vec::new();
    while let Some(row) = rows.next()? {
        notes.push(row_to_note(row)?);
    }
    Ok(notes)
}
