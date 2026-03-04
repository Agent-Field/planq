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
        _ => match init_db(&cli.db).and_then(|db| planq::cli::run(&db, cli)) {
            Ok(()) => {}
            Err(err) => {
                eprintln!("error: {err}");
                std::process::exit(1);
            }
        },
    }
}
