use crate::db::Database;
use crate::models::TaskStatus;
use anyhow::Result;
use rusqlite::params;

#[derive(Debug, Clone, serde::Serialize)]
pub struct FileConflict {
    pub path: String,
    pub task_id: String,
    pub agent_id: Option<String>,
    pub status: TaskStatus,
}

const INSERT_TASK_FILE: &str = "INSERT OR IGNORE INTO task_files(task_id, path) VALUES (?1, ?2);";
const LIST_TASK_FILES: &str = "SELECT path FROM task_files WHERE task_id = ?1 ORDER BY path ASC;";
const CHECK_FILE_CONFLICTS: &str = r#"
SELECT tf.path, t.id, t.agent_id, t.status
FROM task_files tf
JOIN tasks t ON t.id = tf.task_id
WHERE t.project_id = ?1
  AND t.status IN ('running', 'claimed')
  AND (?2 IS NULL OR t.id != ?2)
ORDER BY tf.path ASC, t.id ASC;
"#;

pub fn add_task_files(db: &Database, task_id: &str, paths: &[String]) -> Result<usize> {
    if paths.is_empty() {
        return Ok(0);
    }
    let conn = db.lock()?;
    let mut inserted = 0usize;
    for path in paths {
        inserted += conn.execute(INSERT_TASK_FILE, params![task_id, path])?;
    }
    Ok(inserted)
}

pub fn list_task_files(db: &Database, task_id: &str) -> Result<Vec<String>> {
    let conn = db.lock()?;
    let mut stmt = conn.prepare(LIST_TASK_FILES)?;
    let mut rows = stmt.query(params![task_id])?;
    let mut files = Vec::new();
    while let Some(row) = rows.next()? {
        files.push(row.get(0)?);
    }
    Ok(files)
}

pub fn check_file_conflicts(
    db: &Database,
    project_id: &str,
    exclude_task_id: Option<&str>,
) -> Result<Vec<FileConflict>> {
    let conn = db.lock()?;
    let mut stmt = conn.prepare(CHECK_FILE_CONFLICTS)?;
    let mut rows = stmt.query(params![project_id, exclude_task_id])?;
    let mut conflicts = Vec::new();
    while let Some(row) = rows.next()? {
        conflicts.push(FileConflict {
            path: row.get(0)?,
            task_id: row.get(1)?,
            agent_id: row.get(2)?,
            status: row.get(3)?,
        });
    }
    Ok(conflicts)
}
