use crate::api::types::SyncMessage;
use crate::models::Fill;
use crate::orderbook::OrderBook;
use std::sync::{Arc, atomic::AtomicU64};
use tokio::sync::{Mutex, broadcast};

#[derive(Clone)]
pub struct Config {
    pub is_primary: bool,
    pub primary_url: Option<String>,
}

impl Config {
    pub fn from_env() -> Self {
        let is_primary = std::env::var("IS_PRIMARY")
            .map(|v| v.to_lowercase() == "true")
            .unwrap_or(true);
        let primary_url = std::env::var("PRIMARY_URL").ok();
        Config {
            is_primary,
            primary_url,
        }
    }
}

#[derive(Clone)]
pub struct AppState {
    pub orderbook: Arc<Mutex<OrderBook>>,
    pub order_id_counter: Arc<AtomicU64>,
    pub config: Config,
    pub fills_tx: broadcast::Sender<Fill>, // For external WebSocket clients
    pub sync_tx: broadcast::Sender<SyncMessage>, // For SECONDARY sync
    pub http_client: reqwest::Client,
}

impl AppState {
    pub fn new(config: Config) -> Self {
        let (fills_tx, _) = broadcast::channel(1024);
        let (sync_tx, _) = broadcast::channel(2048);
        AppState {
            orderbook: Arc::new(Mutex::new(OrderBook::new())),
            order_id_counter: Arc::new(AtomicU64::new(0)),
            config,
            fills_tx,
            sync_tx,
            http_client: reqwest::Client::new(),
        }
    }

    pub fn next_order_id(&self) -> u64 {
        self.order_id_counter
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    }
}
