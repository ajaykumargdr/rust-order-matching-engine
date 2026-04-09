use serde::{Deserialize, Serialize};

use crate::models::Fill;

#[derive(Clone, Deserialize, Serialize)]
pub struct CreateOrderRequest {
    pub symbol: String,
    pub side: String,
    pub price: u64,
    pub qty: u64,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct CreateOrderResponse {
    pub order_id: u64,
    pub fills: Vec<Fill>,
}

#[derive(Serialize, Clone, Deserialize)]
pub struct OrderLevel {
    pub price: u64,
    pub qty: u64,
}

#[derive(Serialize)]
pub struct OrderBookResponse {
    pub symbol: String,
    pub bids: Vec<OrderLevel>,
    pub asks: Vec<OrderLevel>,
}

#[derive(Serialize)]
pub struct ErrorResponse {
    pub error: String,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(tag = "type")]
pub enum SyncMessage {
    Snapshot {
        symbol: String,
        bids: Vec<OrderLevel>,
        asks: Vec<OrderLevel>,
    },
    BidUpdate {
        symbol: String,
        price: u64,
        qty: u64,
    },
    AskUpdate {
        symbol: String,
        price: u64,
        qty: u64,
    },
    Fill {
        symbol: String,
        data: Fill,
    },
}

#[derive(Deserialize)]
pub struct SymbolQuery {
    pub symbol: String,
}
