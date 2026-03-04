use crate::db::{json_to_sql, parse_json, Database};
use crate::models::{Dependency, DependencyCondition, DependencyKind};
use anyhow::Result;
use rusqlite::params;

const INSERT_DEPENDENCY: &str = r#"
INSERT INTO dependencies(from_task, to_task, kind, condition, metadata)
VALUES (?1, ?2, ?3, ?4, ?5);
"#;

const SELECT_DEPENDENCY_BY_ID: &str = r#"
SELECT id, from_task, to_task, kind, condition, metadata
FROM dependencies
WHERE id = ?1;
"#;

const REMOVE_DEPENDENCY: &str = "DELETE FROM dependencies WHERE from_task = ?1 AND to_task = ?2;";

const LIST_DEPENDENCIES_FOR_TASK: &str = r#"
SELECT id, from_task, to_task, kind, condition, metadata
FROM dependencies
WHERE from_task = ?1 OR to_task = ?1
ORDER BY id ASC;
"#;

const GET_UPSTREAM_TASKS: &str = r#"
SELECT from_task FROM dependencies WHERE to_task = ?1 ORDER BY id ASC;
"#;

const GET_DOWNSTREAM_TASKS: &str = r#"
SELECT to_task FROM dependencies WHERE from_task = ?1 ORDER BY id ASC;
"#;

fn row_to_dependency(row: &rusqlite::Row<'_>) -> rusqlite::Result<Dependency> {
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
    let metadata: Option<String> = row.get(5)?;
    Ok(Dependency {
        id: row.get(0)?,
        from_task: row.get(1)?,
        to_task: row.get(2)?,
        kind: row.get::<_, DependencyKind>(3)?,
        condition: row.get::<_, DependencyCondition>(4)?,
        metadata: parse_json(metadata).map_err(|e| conv(5, e))?,
    })
}

pub fn add_dependency(
    db: &Database,
    from_task: &str,
    to_task: &str,
    kind: DependencyKind,
    condition: DependencyCondition,
    metadata: Option<serde_json::Value>,
) -> Result<Dependency> {
    let conn = db.lock()?;
    let metadata = json_to_sql(&metadata)?;
    conn.execute(
        INSERT_DEPENDENCY,
        params![from_task, to_task, kind, condition, metadata],
    )?;
    let id = conn.last_insert_rowid();
    let mut stmt = conn.prepare(SELECT_DEPENDENCY_BY_ID)?;
    let dep = stmt.query_row(params![id], row_to_dependency)?;
    Ok(dep)
}

pub fn remove_dependency(db: &Database, from_task: &str, to_task: &str) -> Result<usize> {
    let conn = db.lock()?;
    Ok(conn.execute(REMOVE_DEPENDENCY, params![from_task, to_task])?)
}

pub fn list_dependencies(db: &Database, task_id: &str) -> Result<Vec<Dependency>> {
    let conn = db.lock()?;
    let mut stmt = conn.prepare(LIST_DEPENDENCIES_FOR_TASK)?;
    let mut rows = stmt.query(params![task_id])?;
    let mut result = Vec::new();
    while let Some(row) = rows.next()? {
        result.push(row_to_dependency(row)?);
    }
    Ok(result)
}

pub fn get_upstream_tasks(db: &Database, task_id: &str) -> Result<Vec<String>> {
    let conn = db.lock()?;
    let mut stmt = conn.prepare(GET_UPSTREAM_TASKS)?;
    let mut rows = stmt.query(params![task_id])?;
    let mut out = Vec::new();
    while let Some(row) = rows.next()? {
        out.push(row.get(0)?);
    }
    Ok(out)
}

pub fn get_downstream_tasks(db: &Database, task_id: &str) -> Result<Vec<String>> {
    let conn = db.lock()?;
    let mut stmt = conn.prepare(GET_DOWNSTREAM_TASKS)?;
    let mut rows = stmt.query(params![task_id])?;
    let mut out = Vec::new();
    while let Some(row) = rows.next()? {
        out.push(row.get(0)?);
    }
    Ok(out)
}
