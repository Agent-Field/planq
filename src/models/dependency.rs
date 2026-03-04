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
pub enum DependencyKind {
    Blocks,
    FeedsInto,
    Suggests,
}

impl Display for DependencyKind {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Blocks => "blocks",
            Self::FeedsInto => "feeds_into",
            Self::Suggests => "suggests",
        })
    }
}

impl FromStr for DependencyKind {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "blocks" => Ok(Self::Blocks),
            "feeds_into" => Ok(Self::FeedsInto),
            "suggests" => Ok(Self::Suggests),
            _ => Err(format!(
                "invalid dependency kind: {s}. Valid: blocks, feeds_into, suggests"
            )),
        }
    }
}

impl ToSql for DependencyKind {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        Ok(ToSqlOutput::from(self.to_string()))
    }
}

impl FromSql for DependencyKind {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        let text = value.as_str()?;
        Self::from_str(text).map_err(parse_err)
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum DependencyCondition {
    All,
    Any,
    AtLeast(u32),
    Percent(u32),
}

impl Display for DependencyCondition {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::All => f.write_str("all"),
            Self::Any => f.write_str("any"),
            Self::AtLeast(v) => write!(f, "at_least:{v}"),
            Self::Percent(v) => write!(f, "percent:{v}"),
        }
    }
}

impl FromStr for DependencyCondition {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s == "all" {
            return Ok(Self::All);
        }
        if s == "any" {
            return Ok(Self::Any);
        }
        if let Some(rest) = s.strip_prefix("at_least:") {
            let value = rest
                .parse::<u32>()
                .map_err(|_| format!("invalid at_least value: {rest}"))?;
            return Ok(Self::AtLeast(value));
        }
        if let Some(rest) = s.strip_prefix("percent:") {
            let value = rest
                .parse::<u32>()
                .map_err(|_| format!("invalid percent value: {rest}"))?;
            return Ok(Self::Percent(value));
        }
        Err(format!("invalid dependency condition: {s}"))
    }
}

impl ToSql for DependencyCondition {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        Ok(ToSqlOutput::from(self.to_string()))
    }
}

impl FromSql for DependencyCondition {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        let text = value.as_str()?;
        Self::from_str(text).map_err(parse_err)
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Dependency {
    pub id: i64,
    pub from_task: String,
    pub to_task: String,
    pub kind: DependencyKind,
    pub condition: DependencyCondition,
    pub metadata: Option<serde_json::Value>,
}
