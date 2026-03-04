use clap::Parser;
use planq::cli::{Cli, Commands};
use planq::db::init_db;

fn main() {
    let cli = Cli::parse();

    match &cli.command {
        Commands::Mcp => {
            if let Err(err) = planq::mcp::run_mcp_server(&cli.db) {
                eprintln!("error: {err}");
                std::process::exit(1);
            }
        }
        Commands::Serve { port } => {
            let db_path = cli.db.clone();
            let port = *port;
            let rt = tokio::runtime::Runtime::new().expect("failed to create tokio runtime");
            if let Err(err) = rt.block_on(planq::server::run_server(&db_path, port)) {
                eprintln!("error: {err}");
                std::process::exit(1);
            }
        }
        Commands::Prompt { r#for, list } => {
            if *list || r#for.is_none() {
                println!("Available platforms:");
                println!("  mcp   — Claude Code, Cursor, Windsurf, any MCP client");
                println!("  cli   — Codex, Aider, any CLI-based agent");
                println!("  http  — OpenRouter, custom agents, any HTTP client");
                println!();
                println!("Usage: planq prompt --for <platform>");
                return;
            }
            match r#for.as_deref().unwrap() {
                "mcp" => print_prompt_mcp(),
                "cli" => print_prompt_cli(&cli.db),
                "http" => print_prompt_http(),
                _ => unreachable!(),
            }
        }
        _ => match init_db(&cli.db).and_then(|db| planq::cli::run(&db, cli)) {
            Ok(()) => {}
            Err(err) => {
                eprintln!("error: {err}");
                std::process::exit(1);
            }
        },
    }
}

fn print_prompt_mcp() {
    println!(
        r#"# MCP Config (Claude Code, Cursor, Windsurf, or any MCP client)
#
# Add to your MCP settings:

{{
  "mcpServers": {{
    "planq": {{
      "command": "planq",
      "args": ["mcp"]
    }}
  }}
}}

# Add to project instructions (CLAUDE.md, .cursorrules, etc.):

Use planq to plan and track work. Decompose into tasks with dependencies before coding.
When multiple tasks are ready, work them in parallel.
- `planq go` — claim next ready task
- `planq done --next` — complete and advance
- Adapt: `planq task insert`, `planq ahead`, `planq what-if`"#
    );
}

fn print_prompt_cli(db_path: &str) {
    println!(
        r#"# System prompt / AGENTS.md addition:

You have planq for task graph management (binary: planq, DB: {db_path}).
Before coding: planq project create "name" then create tasks with --dep for dependencies.
Work loop: planq go --agent $AGENT → implement → planq done --next.
When parallel tasks are ready, spawn separate agents for each.
Adapt mid-flight: planq task insert, planq ahead, planq what-if.
planq --help for full reference."#
    );
}

fn print_prompt_http() {
    println!(
        r#"# Start the Planq server first:
#   planq serve --port 8080

# System prompt addition:

You have a task management API at http://localhost:8080.
POST /projects — create project. POST /tasks — create tasks with deps.
POST /go — claim next ready task. POST /tasks/:id/done — complete.
POST /tasks/insert — add steps. GET /ahead — preview upcoming.
Decompose work into tasks with dependencies. Adapt as you learn.
When parallel tasks are ready, work them in parallel."#
    );
}
