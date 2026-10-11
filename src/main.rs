mod cli;
mod config;
mod error;
mod logger;
mod server;
mod syncer;

use tokio_util::sync::CancellationToken;

#[tokio::main]
async fn main() {
    let cfg = config::load();

    let _ = logger::init(cfg.log_level);

    let token = CancellationToken::new();

    server::run(token).await;
}
