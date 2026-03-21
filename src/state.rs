use crate::orderbook::OrderBook;
use std::sync::{Arc, Mutex, atomic::AtomicU64};

#[derive(Clone)]
pub struct AppState {
    pub orderbook: Arc<Mutex<OrderBook>>,
    pub order_id_counter: Arc<AtomicU64>,
}

impl AppState {
    pub fn new() -> Self {
        AppState {
            orderbook: Arc::new(Mutex::new(OrderBook::new())),
            order_id_counter: Arc::new(AtomicU64::new(0)),
        }
    }

    pub fn next_order_id(&self) -> u64 {
        self.order_id_counter
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    }
}
