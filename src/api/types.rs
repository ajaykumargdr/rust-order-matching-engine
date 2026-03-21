use serde::{Deserialize, Serialize};

use crate::models::Fill;

#[derive(Clone, Deserialize, Serialize)]
pub struct CreateOrderRequest {
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
        bids: Vec<OrderLevel>,
        asks: Vec<OrderLevel>,
    },
    BidUpdate {
        price: u64,
        qty: u64,
    },
    AskUpdate {
        price: u64,
        qty: u64,
    },
    Fill(Fill),
}
