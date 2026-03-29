use crate::models::{Fill, Order, Side};
use crate::orderbook::OrderBook;

#[derive(Debug, Clone)]
pub struct OrderBookDelta {
    pub side: Side,
    pub price: u64,
    pub qty: u64,
}

pub struct ProcessResult {
    pub fills: Vec<Fill>,
    pub deltas: Vec<OrderBookDelta>,
}

pub fn process_order(mut incoming: Order, book: &mut OrderBook) -> ProcessResult {
    let mut fills = Vec::new();
    let mut deltas = Vec::new();

    match incoming.side {
        Side::Buy => {
            // Match with lowest asks
            while incoming.qty > 0 {
                let (best_ask_price, orders) = match book.asks.first_key_value() {
                    Some((price, orders)) => (*price, orders),
                    None => break,
                };

                if incoming.price < best_ask_price {
                    break;
                }

                let prev_qty: u64 = orders.iter().map(|o| o.qty).sum();
                let queue = book.asks.get_mut(&best_ask_price).unwrap();
                let mut level_traded_qty = 0;

                while let Some(mut ask_order) = queue.pop_front() {
                    let traded_qty = incoming.qty.min(ask_order.qty);
                    level_traded_qty += traded_qty;

                    fills.push(Fill {
                        maker_order_id: ask_order.id,
                        taker_order_id: incoming.id,
                        price: best_ask_price,
                        qty: traded_qty,
                    });

                    incoming.qty -= traded_qty;
                    ask_order.qty -= traded_qty;

                    if ask_order.qty > 0 {
                        queue.push_front(ask_order);
                        break;
                    }

                    if incoming.qty == 0 {
                        break;
                    }
                }

                if queue.is_empty() {
                    book.asks.remove(&best_ask_price);
                    // Level removed - qty is now 0
                    deltas.push(OrderBookDelta {
                        side: Side::Sell,
                        price: best_ask_price,
                        qty: 0,
                    });
                } else {
                    let new_qty = prev_qty - level_traded_qty;

                    // Only send delta if qty changed
                    if new_qty != prev_qty {
                        deltas.push(OrderBookDelta {
                            side: Side::Sell,
                            price: best_ask_price,
                            qty: new_qty,
                        });
                    }
                }
            }

            // Add resting order
            if incoming.qty > 0 {
                let price = incoming.price;
                book.add_order(incoming).unwrap();

                // Calculate total qty at this price level
                let qty: u64 = book
                    .bids
                    .get(&price)
                    .map(|q| q.iter().map(|o| o.qty).sum())
                    .unwrap_or(0);

                deltas.push(OrderBookDelta {
                    side: Side::Buy,
                    price,
                    qty,
                });
            }
        }

        Side::Sell => {
            // Match with highest bids
            while incoming.qty > 0 {
                let (best_bid_price, orders) = match book.bids.last_key_value() {
                    Some((price, orders)) => (*price, orders),
                    None => break,
                };

                if incoming.price > best_bid_price {
                    break;
                }

                let prev_qty: u64 = orders.iter().map(|o| o.qty).sum();
                let queue = book.bids.get_mut(&best_bid_price).unwrap();
                let mut level_traded_qty = 0;

                while let Some(mut bid_order) = queue.pop_front() {
                    let traded_qty = incoming.qty.min(bid_order.qty);
                    level_traded_qty += traded_qty;

                    fills.push(Fill {
                        maker_order_id: bid_order.id,
                        taker_order_id: incoming.id,
                        price: best_bid_price,
                        qty: traded_qty,
                    });

                    incoming.qty -= traded_qty;
                    bid_order.qty -= traded_qty;

                    if bid_order.qty > 0 {
                        queue.push_front(bid_order);
                        break;
                    }

                    if incoming.qty == 0 {
                        break;
                    }
                }

                if queue.is_empty() {
                    book.bids.remove(&best_bid_price);
                    // Level removed - qty is now 0
                    deltas.push(OrderBookDelta {
                        side: Side::Buy,
                        price: best_bid_price,
                        qty: 0,
                    });
                } else {
                    let new_qty = prev_qty - level_traded_qty;

                    // Only send delta if qty changed
                    if new_qty != prev_qty {
                        deltas.push(OrderBookDelta {
                            side: Side::Buy,
                            price: best_bid_price,
                            qty: new_qty,
                        });
                    }
                }
            }

            // Add resting order
            if incoming.qty > 0 {
                let price = incoming.price;
                book.add_order(incoming.clone()).unwrap();

                // Calculate total qty at this price level
                let qty: u64 = book
                    .asks
                    .get(&price)
                    .map(|q| q.iter().map(|o| o.qty).sum())
                    .unwrap_or(0);

                deltas.push(OrderBookDelta {
                    side: Side::Sell,
                    price,
                    qty,
                });
            }
        }
    }

    ProcessResult { fills, deltas }
}
