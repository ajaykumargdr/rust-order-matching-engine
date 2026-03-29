use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::{extract::State, response::IntoResponse};
use futures_util::{SinkExt, StreamExt};
use tokio_tungstenite::tungstenite::Message as WsMessage;

use crate::{
    models::{Order, Side},
    orderbook::OrderBook,
    state::AppState,
};

use super::types::SyncMessage;

pub async fn ws_handler(ws: WebSocketUpgrade, State(state): State<AppState>) -> impl IntoResponse {
    ws.on_upgrade(|socket| handle_ws_socket(socket, state))
}

pub async fn internal_sync(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> impl IntoResponse {
    ws.on_upgrade(|socket| handle_sync_socket(socket, state))
}

/// Handler for external WebSocket clients - receives fills only (no deltas)
async fn handle_ws_socket(socket: WebSocket, state: AppState) {
    let (mut sender, mut receiver) = socket.split();

    println!("[WS] External client connected");

    // External clients subscribe to fills channel (fills only, no deltas)
    let mut rx = state.fills_tx.subscribe();

    loop {
        tokio::select! {
            Ok(fill) = rx.recv() => {
                let json = serde_json::to_string(&fill).ok();
                if let Some(text) = json
                    && sender.send(Message::Text(text.into())).await.is_err()
                {
                    println!("[WS] External client disconnected (send error)");
                    return;
                }
            }
            msg = receiver.next() => {
                match msg {
                    Some(Ok(Message::Close(_))) => {
                        println!("[WS] External client closed connection");
                        return;
                    }
                    Some(Err(e)) => {
                        println!("[WS] External client error: {}", e);
                        return;
                    }
                    Some(Ok(_)) => {}
                    None => {
                        println!("[WS] External client disconnected (connection dropped)");
                        return;
                    }
                }
            }
        }
    }
}

async fn handle_sync_socket(socket: WebSocket, state: AppState) {
    let (mut sender, mut receiver) = socket.split();

    println!("[WS] Secondary sync client connected");

    // SECONDARY instances subscribe to sync channel (fills + deltas)
    let mut rx = state.sync_tx.subscribe();

    // Send initial snapshot on connect
    let orderbook_state = super::orderbook_state(&state).await;
    let snapshot = SyncMessage::Snapshot {
        bids: orderbook_state.0,
        asks: orderbook_state.1,
    };

    let json = serde_json::to_string(&snapshot).ok();
    if let Some(text) = json {
        let _ = sender.send(Message::Text(text.into())).await;
    }

    loop {
        tokio::select! {
            Ok(msg) = rx.recv() => {
                let json = serde_json::to_string(&msg).ok();
                if let Some(text) = json
                    && sender.send(Message::Text(text.into())).await.is_err()
                {
                    println!("[WS] Secondary sync client disconnected (send error)");
                    return;
                }
            }
            msg = receiver.next() => {
                match msg {
                    Some(Ok(Message::Close(_))) => {
                        println!("[WS] Secondary sync client closed connection");
                        return;
                    }
                    Some(Err(e)) => {
                        println!("[WS] Secondary sync client error: {}", e);
                        return;
                    }
                    Some(Ok(_)) => {}
                    None => {
                        println!("[WS] Secondary sync client disconnected (connection dropped)");
                        return;
                    }
                }
            }
        }
    }
}

pub async fn sync_from_primary(state: AppState) {
    let primary_url = match &state.config.primary_url {
        Some(url) => url,
        None => return,
    };

    // SECONDARY connects to /internal/sync for full sync messages (fills + deltas)
    let ws_url = primary_url
        .replace("http://", "ws://")
        .replace("https://", "wss://")
        + "/internal/sync";

    loop {
        println!("[SECONDARY] Connecting to PRIMARY at {}", ws_url);

        match tokio_tungstenite::connect_async(&ws_url).await {
            Ok((ws_stream, _)) => {
                println!("[SECONDARY] Connected to PRIMARY");
                handle_primary_ws(ws_stream, state.clone()).await;
                println!("[SECONDARY] Disconnected from PRIMARY, reconnecting...");
            }
            Err(e) => {
                println!("[SECONDARY] Connection failed: {}, retrying in 5s", e);
            }
        }

        tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
    }
}

async fn handle_primary_ws(
    ws_stream: tokio_tungstenite::WebSocketStream<
        tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
    >,
    state: AppState,
) {
    let (_write, mut read) = ws_stream.split();

    println!("[SECONDARY] Connected to PRIMARY WebSocket");

    while let Some(msg) = read.next().await {
        match msg {
            Ok(WsMessage::Text(text)) => {
                if let Ok(sync_msg) = serde_json::from_str::<SyncMessage>(&text) {
                    match &sync_msg {
                        SyncMessage::Fill(fill) => {
                            // Forward fills to local WebSocket clients
                            let _ = state.fills_tx.send(fill.clone());
                        }
                        SyncMessage::BidUpdate { price, qty } => {
                            let mut ob = state.orderbook.lock().await;
                            update_level(&mut ob, Side::Buy, *price, *qty);
                        }
                        SyncMessage::AskUpdate { price, qty } => {
                            let mut ob = state.orderbook.lock().await;
                            update_level(&mut ob, Side::Sell, *price, *qty);
                        }
                        SyncMessage::Snapshot { bids, asks } => {
                            let mut ob = state.orderbook.lock().await;
                            *ob = OrderBook::new();

                            // Note: For Secondary instances we only store order price and total quantity
                            for level in bids {
                                let order = Order {
                                    id: 0,
                                    side: Side::Buy,
                                    price: level.price,
                                    qty: level.qty,
                                };
                                ob.add_order(order).unwrap();
                            }
                            for level in asks {
                                let order = Order {
                                    id: 0,
                                    side: Side::Sell,
                                    price: level.price,
                                    qty: level.qty,
                                };
                                ob.add_order(order).unwrap();
                            }
                        }
                    }
                }
            }
            Ok(WsMessage::Close(reason)) => {
                println!("[SECONDARY] PRIMARY closed connection: {:?}", reason);
                break;
            }
            Err(e) => {
                println!("[SECONDARY] Connection error: {}", e);
                break;
            }
            _ => {}
        }
    }

    println!("[SECONDARY] Disconnected from PRIMARY WebSocket");
}

/// Update orderbook level from delta message
fn update_level(ob: &mut OrderBook, side: Side, price: u64, qty: u64) {
    if qty == 0 {
        // Remove price level
        match side {
            Side::Buy => {
                ob.bids.remove(&price);
            }
            Side::Sell => {
                ob.asks.remove(&price);
            }
        }
    } else {
        // Update/add price level
        let queue = match side {
            Side::Buy => ob.bids.entry(price).or_default(),
            Side::Sell => ob.asks.entry(price).or_default(),
        };

        queue.clear();
        queue.push_back(Order {
            id: 0,
            side,
            price,
            qty,
        });
    }
}
