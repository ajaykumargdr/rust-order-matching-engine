mod api;
mod engine;
mod models;
mod orderbook;
mod state;

use axum::{
    Router,
    routing::{get, post},
};
use state::{AppState, Config};

#[tokio::main]
async fn main() {
    let config = Config::from_env();
    let mode = if config.is_primary {
        "PRIMARY"
    } else {
        "SECONDARY"
    };
    let port = std::env::var("PORT").unwrap_or_else(|_| "3000".to_string());
    let addr = format!("0.0.0.0:{}", port);

    println!("[{}] Starting server on {}", mode, addr);
    if !config.is_primary
        && let Some(ref url) = config.primary_url
    {
        println!("[{}] Will sync from PRIMARY at {}", mode, url);
    }

    let state = AppState::new(config);

    // Start sync task for SECONDARY instances
    if !state.config.is_primary {
        let sync_state = state.clone();
        tokio::spawn(async move {
            api::sync_from_primary(sync_state).await;
        });
    }

    let app: Router = Router::new()
        .route("/orders", post(api::create_order))
        .route("/orderbook", get(api::get_orderbook))
        .route("/ws", get(api::ws_handler)) // External WebSocket clients
        .route("/internal/sync", get(api::internal_sync)) // SECONDARY sync
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
