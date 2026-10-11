use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(version, about = "Hakkawine Agent CLI tool")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Server,
}
