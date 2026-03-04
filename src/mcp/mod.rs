pub mod protocol;
pub mod server;
pub mod tools;

use anyhow::Result;

pub fn run_mcp_server(db_path: &str) -> Result<()> {
    server::run_server(db_path)
}
