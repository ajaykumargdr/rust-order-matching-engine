mod types;

use axum::{Json, extract::State, http::StatusCode};

use crate::{
    engine,
    models::{Order, Side},
    state::AppState,
};
use types::*;

pub async fn create_order(
    State(state): State<AppState>,
    Json(payload): Json<CreateOrderRequest>,
) -> Result<Json<CreateOrderResponse>, (StatusCode, Json<ErrorResponse>)> {
    let side = match payload.side.to_lowercase().as_str() {
        "buy" => Side::Buy,
        "sell" => Side::Sell,
        _ => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: "invalid side".into(),
                }),
            ));
        }
    };

    if payload.price == 0 || payload.qty == 0 {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "price and qty must be > 0".into(),
            }),
        ));
    }

    let mut ob = state.orderbook.lock().unwrap();
    let order_id = state.next_order_id();

    let order = Order {
        id: order_id,
        side,
        price: payload.price,
        qty: payload.qty,
    };

    let fills = engine::process_order(order, &mut ob);

    let fills_response = fills
        .into_iter()
        .map(|f| FillResponse {
            maker_order_id: f.maker_order_id,
            taker_order_id: f.taker_order_id,
            price: f.price,
            qty: f.qty,
        })
        .collect();

    Ok(Json(CreateOrderResponse {
        order_id,
        fills: fills_response,
    }))
}

pub async fn get_orderbook(State(state): State<AppState>) -> Json<OrderBookResponse> {
    let ob = state.orderbook.lock().unwrap();

    // Bids in descending order (highest bid first)
    let bids = ob
        .bids
        .iter()
        .rev()
        .map(|(&price, orders)| OrderLevel {
            price,
            qty: orders.iter().map(|o| o.qty).sum(),
        })
        .collect();

    // Asks in ascending order (lowest ask first)
    let asks = ob
        .asks
        .iter()
        .map(|(&price, orders)| OrderLevel {
            price,
            qty: orders.iter().map(|o| o.qty).sum(),
        })
        .collect();

    Json(OrderBookResponse { bids, asks })
}
