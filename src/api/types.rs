use serde::{Deserialize, Serialize};

#[derive(Clone, Deserialize)]
pub struct CreateOrderRequest {
    pub side: String,
    pub price: u64,
    pub qty: u64,
}

#[derive(Serialize)]
pub struct ErrorResponse {
    pub error: String,
}

#[derive(Clone, Serialize)]
pub struct CreateOrderResponse {
    pub order_id: u64,
    pub fills: Vec<FillResponse>,
}

#[derive(Clone, Serialize)]
pub struct FillResponse {
    pub maker_order_id: u64,
    pub taker_order_id: u64,
    pub price: u64,
    pub qty: u64,
}

#[derive(Serialize)]
pub struct OrderBookResponse {
    pub bids: Vec<OrderLevel>,
    pub asks: Vec<OrderLevel>,
}

#[derive(Serialize)]
pub struct OrderLevel {
    pub price: u64,
    pub qty: u64,
}
