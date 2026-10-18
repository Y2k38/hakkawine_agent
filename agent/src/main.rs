mod cli;
mod config;
mod error;
mod logger;
mod server;
mod syncer;

#[tokio::main]
async fn main() {
    let cfg = config::load();

    let _ = logger::init(cfg.log_level.clone());

    cli::run(cfg).await;
}
