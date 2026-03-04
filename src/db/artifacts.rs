use crate::db::{dt_to_sql, json_to_sql, parse_dt, parse_json, Database};
use crate::models::Artifact;
use anyhow::Result;
use rusqlite::params;

const INSERT_ARTIFACT: &str = r#"
INSERT INTO artifacts(id, task_id, name, kind, content, path, size_bytes, mime_type, metadata, created_at)
VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10);
"#;

const SELECT_ARTIFACT_BY_ID: &str = r#"
SELECT id, task_id, name, kind, content, path, size_bytes, mime_type, metadata, created_at
FROM artifacts
WHERE id = ?1;
"#;

const LIST_ARTIFACTS_BY_TASK: &str = r#"
SELECT id, task_id, name, kind, content, path, size_bytes, mime_type, metadata, created_at
FROM artifacts
WHERE task_id = ?1
ORDER BY created_at ASC;
"#;

const GET_UPSTREAM_ARTIFACTS: &str = r#"
SELECT a.id, a.task_id, a.name, a.kind, a.content, a.path, a.size_bytes, a.mime_type, a.metadata, a.created_at
FROM artifacts a
JOIN dependencies d ON d.from_task = a.task_id
WHERE d.to_task = ?1 AND d.kind = 'feeds_into'
ORDER BY a.created_at ASC;
"#;

fn row_to_artifact(row: &rusqlite::Row<'_>) -> rusqlite::Result<Artifact> {
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
    let metadata: Option<String> = row.get(8)?;
    Ok(Artifact {
        id: row.get(0)?,
        task_id: row.get(1)?,
        name: row.get(2)?,
        kind: row.get(3)?,
        content: row.get(4)?,
        path: row.get(5)?,
        size_bytes: row.get(6)?,
        mime_type: row.get(7)?,
        metadata: parse_json(metadata).map_err(|e| conv(8, e))?,
        created_at: parse_dt(row.get::<_, String>(9)?).map_err(|e| conv(9, e))?,
    })
}

pub fn create_artifact(db: &Database, artifact: &Artifact) -> Result<Artifact> {
    let conn = db.lock()?;
    conn.execute(
        INSERT_ARTIFACT,
        params![
            &artifact.id,
            &artifact.task_id,
            &artifact.name,
            &artifact.kind,
            &artifact.content,
            &artifact.path,
            artifact.size_bytes,
            &artifact.mime_type,
            json_to_sql(&artifact.metadata)?,
            dt_to_sql(artifact.created_at),
        ],
    )?;
    drop(conn);
    get_artifact(db, &artifact.id)
}

pub fn get_artifact(db: &Database, artifact_id: &str) -> Result<Artifact> {
    let conn = db.lock()?;
    let mut stmt = conn.prepare(SELECT_ARTIFACT_BY_ID)?;
    let artifact = stmt.query_row(params![artifact_id], row_to_artifact)?;
    Ok(artifact)
}

pub fn list_artifacts(db: &Database, task_id: &str) -> Result<Vec<Artifact>> {
    let conn = db.lock()?;
    let mut stmt = conn.prepare(LIST_ARTIFACTS_BY_TASK)?;
    let mut rows = stmt.query(params![task_id])?;
    let mut result = Vec::new();
    while let Some(row) = rows.next()? {
        result.push(row_to_artifact(row)?);
    }
    Ok(result)
}

pub fn get_upstream_artifacts(db: &Database, task_id: &str) -> Result<Vec<Artifact>> {
    let conn = db.lock()?;
    let mut stmt = conn.prepare(GET_UPSTREAM_ARTIFACTS)?;
    let mut rows = stmt.query(params![task_id])?;
    let mut result = Vec::new();
    while let Some(row) = rows.next()? {
        result.push(row_to_artifact(row)?);
    }
    Ok(result)
}
