pub mod types;
pub mod ws;

use axum::{Json, extract::State, http::StatusCode};

use crate::{
    engine,
    models::{Order, Side},
    state::AppState,
};

use types::*;
pub use ws::*;

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

    if state.config.is_primary {
        let mut ob = state.orderbook.lock().await;
        let order_id = state.next_order_id();

        let order = Order {
            id: order_id,
            side: side.clone(),
            price: payload.price,
            qty: payload.qty,
        };

        let result = engine::process_order(order, &mut ob);
        drop(ob);

        // Broadcast fills to both channels in single iteration
        for fill in &result.fills {
            let _ = state.fills_tx.send(fill.clone()); // External WS clients
            let _ = state.sync_tx.send(SyncMessage::Fill(fill.clone())); // SECONDARY sync
        }

        // Broadcast deltas to SECONDARY sync channel
        for delta in &result.deltas {
            let msg = match delta.side {
                Side::Buy => SyncMessage::BidUpdate {
                    price: delta.price,
                    qty: delta.qty,
                },
                Side::Sell => SyncMessage::AskUpdate {
                    price: delta.price,
                    qty: delta.qty,
                },
            };
            let _ = state.sync_tx.send(msg);
        }

        Ok(Json(CreateOrderResponse {
            order_id,
            fills: result.fills.clone(),
        }))
    } else {
        let primary_url = state.config.primary_url.as_ref().ok_or((
            StatusCode::BAD_GATEWAY,
            Json(ErrorResponse {
                error: "no primary configured".into(),
            }),
        ))?;

        let url = format!("{}/orders", primary_url.trim_end_matches('/'));
        let resp = state
            .http_client
            .post(&url)
            .json(&payload)
            .send()
            .await
            .map_err(|e| {
                (
                    StatusCode::BAD_GATEWAY,
                    Json(ErrorResponse {
                        error: e.to_string(),
                    }),
                )
            })?;

        if !resp.status().is_success() {
            let err = resp.text().await.unwrap_or_default();
            return Err((StatusCode::BAD_GATEWAY, Json(ErrorResponse { error: err })));
        }

        let result: CreateOrderResponse = resp.json().await.map_err(|e| {
            (
                StatusCode::BAD_GATEWAY,
                Json(ErrorResponse {
                    error: e.to_string(),
                }),
            )
        })?;

        Ok(Json(result))
    }
}

/// Returns (bids, asks)
async fn orderbook_state(state: &AppState) -> (Vec<OrderLevel>, Vec<OrderLevel>) {
    let ob = state.orderbook.lock().await;

    let bids = ob
        .bids
        .iter()
        .rev()
        .map(|(&price, orders)| types::OrderLevel {
            price,
            qty: orders.iter().map(|o| o.qty).sum(),
        })
        .collect();

    let asks = ob
        .asks
        .iter()
        .map(|(&price, orders)| types::OrderLevel {
            price,
            qty: orders.iter().map(|o| o.qty).sum(),
        })
        .collect();

    (bids, asks)
}

pub async fn get_orderbook(State(state): State<AppState>) -> Json<OrderBookResponse> {
    let (bids, asks) = orderbook_state(&state).await;
    Json(OrderBookResponse { bids, asks })
}
