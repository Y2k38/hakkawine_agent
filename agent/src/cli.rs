use clap::{Parser, Subcommand};

use crate::config;
use crate::syncer;

#[derive(Parser)]
#[command(version, about = "Hakkawine Agent CLI tool")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    Syncer,
    Server,
}

pub async fn run(cfg: config::Config) {
    let args = Cli::parse();

    match &args.command {
        Some(Commands::Syncer {}) => {
            syncer::run(cfg).await;
        }
        Some(Commands::Server {}) => {
            println!("Server running");
        }
        None => {
            println!("There was no subcommand given");
        }
    }
}
