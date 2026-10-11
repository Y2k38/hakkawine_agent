use axum::{Router, routing::get};

use crate::server::handlers;

pub fn new() -> Router {
    Router::new().route("/api/v1/list_subs", get(handlers::list_subs))
}
