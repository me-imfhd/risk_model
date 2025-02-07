use axum::{routing::get, Router};
use tracing::{info, Level};

mod kamino;
mod liquidity_risk;
mod risk_model;
mod volatility_risk;

#[tokio::main]
async fn main() {
    dotenv::dotenv().ok();
    tracing_subscriber::fmt()
        .with_target(false)
        .with_level(true)
        .with_file(true)
        .with_line_number(true)
        .with_thread_ids(true)
        .with_max_level(Level::INFO)
        .init();

    let app = Router::new()
        .route("/", get(|| async { "Hello, World!" }))
        .route("/risk_model", get(risk_model::risk_model));

    let listener = tokio::net::TcpListener::bind("0.0.0.0:8000")
        .await
        .expect("Failed to bind to port 8000");
    info!(
        "🚀 Server running on http://{}",
        listener.local_addr().unwrap()
    );
    axum::serve(listener, app).await.expect("Failed to serve");
}
