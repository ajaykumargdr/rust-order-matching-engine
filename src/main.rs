mod api;
mod engine;
mod models;
mod orderbook;
mod state;

use axum::{
    Router,
    routing::{get, post},
};

#[tokio::main]
async fn main() {
    let primary_url = "localhost:3000".to_string();
    let state = state::AppState::new();

    let app: Router = Router::new()
        .route("/orders", post(api::create_order))
        .route("/orderbook", get(api::get_orderbook))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(primary_url).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
