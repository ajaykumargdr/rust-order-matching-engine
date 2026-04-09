use std::collections::HashMap;
use std::sync::{Arc, atomic::AtomicU64};
use tokio::sync::{Mutex, broadcast};

use crate::api::types::SyncMessage;
use crate::models::Fill;
use crate::orderbook::OrderBook;

#[derive(Debug, Clone, serde::Deserialize)]
pub struct ConfigData {
    pub symbols: Vec<String>,
}

#[derive(Clone)]
pub struct Config {
    pub is_primary: bool,
    pub primary_url: Option<String>,
    pub symbols: Vec<String>,
}

pub struct Engine {
    pub books: HashMap<String, OrderBook>,
}

#[derive(Clone)]
pub struct AppState {
    pub engine: Arc<Mutex<Engine>>,
    pub order_id_counter: Arc<AtomicU64>,
    pub config: Config,
    pub fills_tx: broadcast::Sender<Fill>, // For external WebSocket clients
    pub sync_tx: broadcast::Sender<SyncMessage>, // For SECONDARY sync
    pub http_client: reqwest::Client,
}

impl Config {
    pub fn from_env() -> Self {
        let is_primary = std::env::var("IS_PRIMARY")
            .map(|v| v.to_lowercase() == "true")
            .unwrap_or(true);
        let primary_url = std::env::var("PRIMARY_URL").ok();

        // Load symbols from config.json
        let config_path =
            std::env::var("CONFIG_PATH").unwrap_or_else(|_| "config.json".to_string());

        let default = vec![
            "BTC".to_string(),
            "ETH".to_string(),
            "SOL".to_string(),
            "XRP".to_string(),
        ];

        let symbols = match std::fs::read_to_string(&config_path) {
            Ok(content) => match serde_json::from_str::<ConfigData>(&content) {
                Ok(config_data) => config_data.symbols,
                Err(_) => {
                    eprintln!("[WARN] Failed to parse config.json, using default symbols");
                    default
                }
            },
            Err(_) => {
                eprintln!("[WARN] config.json not found, using default symbols");
                default
            }
        };

        Config {
            is_primary,
            primary_url,
            symbols,
        }
    }

    pub fn is_valid_symbol(&self, symbol: &str) -> bool {
        self.symbols.iter().any(|s| s == symbol)
    }
}

impl Engine {
    pub fn new(symbols: &[String]) -> Self {
        let books = symbols
            .iter()
            .cloned()
            .fold(HashMap::new(), |mut acc, symbol| {
                acc.insert(symbol, OrderBook::new());
                acc
            });

        Engine { books }
    }

    pub fn get_orderbook(&self, symbol: &str) -> Option<&OrderBook> {
        self.books.get(symbol)
    }

    pub fn get_orderbook_mut(&mut self, symbol: &str) -> Option<&mut OrderBook> {
        self.books.get_mut(symbol)
    }
}

impl AppState {
    pub fn new(config: Config) -> Self {
        let (fills_tx, _) = broadcast::channel(1024);
        let (sync_tx, _) = broadcast::channel(2048);
        AppState {
            engine: Arc::new(Mutex::new(Engine::new(&config.symbols))),
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

#[cfg(test)]
mod tests {
    use super::{AppState, Config};
    use std::collections::HashSet;
    use std::sync::Arc;
    use tokio::task::JoinSet;

    fn create_test_config() -> Config {
        Config {
            is_primary: true,
            primary_url: None,
            symbols: vec!["BTC".to_string(), "ETH".to_string()],
        }
    }

    #[tokio::test]
    async fn next_order_id_sequential() {
        let config = create_test_config();
        let state = Arc::new(AppState::new(config));
        let mut tasks: JoinSet<()> = JoinSet::new();

        for _ in 0..5 {
            let state = state.clone();
            tasks.spawn(async move {
                state.next_order_id();
            });
        }

        let _ = tasks.join_all().await;

        assert_eq!(state.next_order_id(), 5);
    }

    #[tokio::test]
    async fn next_order_id_unique() {
        let config = create_test_config();
        let state = Arc::new(AppState::new(config));
        let mut tasks: JoinSet<u64> = JoinSet::new();
        let mut unique_ids: HashSet<u64> = HashSet::new();

        for _ in 0..5 {
            let state = state.clone();
            tasks.spawn(async move { state.next_order_id() });
        }

        while let Some(id) = tasks.join_next().await {
            unique_ids.insert(id.unwrap());
        }

        assert_eq!(unique_ids.len(), 5);
    }

    #[tokio::test]
    async fn engine_has_multiple_orderbooks() {
        let config = create_test_config();
        let state = AppState::new(config);
        let engine = state.engine.lock().await;

        assert!(engine.books.contains_key("BTC"));
        assert!(engine.books.contains_key("ETH"));
        assert_eq!(engine.books.len(), 2);
    }

    #[test]
    fn config_validates_symbol() {
        let config = Config {
            is_primary: true,
            primary_url: None,
            symbols: vec!["BTC".to_string(), "ETH".to_string()],
        };

        assert!(config.is_valid_symbol("BTC"));
        assert!(config.is_valid_symbol("ETH"));
        assert!(!config.is_valid_symbol("SOL"));
    }
}
