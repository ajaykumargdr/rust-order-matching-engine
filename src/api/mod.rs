pub mod types;
pub mod ws;

use axum::{
    Json,
    extract::{Query, State},
    http::StatusCode,
};

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

    // Validate symbol
    if !state.config.is_valid_symbol(&payload.symbol) {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: format!(
                    "invalid symbol: {}. Valid symbols: {:?}",
                    payload.symbol, state.config.symbols
                ),
            }),
        ));
    }

    if payload.price == 0 || payload.qty == 0 {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "price and qty must be > 0".into(),
            }),
        ));
    }

    if state.config.is_primary {
        let mut engine = state.engine.lock().await;
        let order_id = state.next_order_id();

        let order = Order {
            id: order_id,
            side: side.clone(),
            price: payload.price,
            qty: payload.qty,
        };

        let book = engine.get_orderbook_mut(&payload.symbol).ok_or((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: "orderbook not found".into(),
            }),
        ))?;

        let result = engine::process_order(order, book);
        drop(engine);

        // Broadcast fills to both channels in single iteration
        for fill in &result.fills {
            let _ = state.fills_tx.send(fill.clone()); // External WS clients
            let _ = state.sync_tx.send(SyncMessage::Fill {
                symbol: payload.symbol.clone(),
                data: fill.clone(),
            }); // SECONDARY sync
        }

        // Broadcast deltas to SECONDARY sync channel
        for delta in &result.deltas {
            let msg = match delta.side {
                Side::Buy => SyncMessage::BidUpdate {
                    symbol: payload.symbol.clone(),
                    price: delta.price,
                    qty: delta.qty,
                },
                Side::Sell => SyncMessage::AskUpdate {
                    symbol: payload.symbol.clone(),
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
async fn orderbook_state(
    state: &AppState,
    symbol: &str,
) -> Result<(Vec<OrderLevel>, Vec<OrderLevel>), (StatusCode, Json<ErrorResponse>)> {
    let engine = state.engine.lock().await;
    let ob = engine.get_orderbook(symbol).ok_or((
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(ErrorResponse {
            error: format!("orderbook for symbol '{}' not found", symbol),
        }),
    ))?;

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

    Ok((bids, asks))
}

pub async fn get_orderbook(
    State(state): State<AppState>,
    Query(query): Query<SymbolQuery>,
) -> Result<Json<OrderBookResponse>, (StatusCode, Json<ErrorResponse>)> {
    // Validate symbol
    if !state.config.is_valid_symbol(&query.symbol) {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: format!(
                    "invalid symbol: {}. Valid symbols: {:?}",
                    query.symbol, state.config.symbols
                ),
            }),
        ));
    }

    let (bids, asks) = orderbook_state(&state, &query.symbol).await?;

    Ok(Json(OrderBookResponse {
        symbol: query.symbol,
        bids,
        asks,
    }))
}
