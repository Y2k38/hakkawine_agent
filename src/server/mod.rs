mod handlers;
mod routes;

use tokio_util::sync::CancellationToken;

pub async fn run(token: CancellationToken) {
    let router = routes::new();

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();

    let _ = axum::serve(listener, router).await.unwrap();
}
