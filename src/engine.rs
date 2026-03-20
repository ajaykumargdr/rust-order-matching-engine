use crate::models::{Fill, Order, Side};
use crate::orderbook::OrderBook;

pub fn process_order(mut incoming: Order, book: &mut OrderBook) -> Vec<Fill> {
    let mut fills = Vec::new();

    match incoming.side {
        Side::Buy => {
            // Match with lowest asks
            while incoming.qty > 0 {
                let best_ask_price = match book.asks.first_key_value() {
                    Some((price, _)) => *price,
                    None => break,
                };

                if incoming.price < best_ask_price {
                    break;
                }

                let queue = book.asks.get_mut(&best_ask_price).unwrap();

                while let Some(mut ask_order) = queue.pop_front() {
                    let traded_qty = incoming.qty.min(ask_order.qty);

                    fills.push(Fill {
                        maker_order_id: ask_order.id,
                        taker_order_id: incoming.id,
                        price: best_ask_price,
                        qty: traded_qty,
                    });

                    incoming.qty -= traded_qty;
                    ask_order.qty -= traded_qty;

                    if ask_order.qty > 0 {
                        // Put remaining back to front
                        queue.push_front(ask_order);
                        break;
                    }

                    if incoming.qty == 0 {
                        break;
                    }
                }

                if queue.is_empty() {
                    book.asks.remove(&best_ask_price);
                }
            }

            if incoming.qty > 0 {
                book.add_order(incoming);
            }
        }

        Side::Sell => {
            // Match with highest bids
            while incoming.qty > 0 {
                let best_bid_price = match book.bids.last_key_value() {
                    Some((price, _)) => *price,
                    None => break,
                };

                if incoming.price > best_bid_price {
                    break;
                }

                let queue = book.bids.get_mut(&best_bid_price).unwrap();

                while let Some(mut bid_order) = queue.pop_front() {
                    let traded_qty = incoming.qty.min(bid_order.qty);

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
                }
            }

            if incoming.qty > 0 {
                book.add_order(incoming);
            }
        }
    }

    fills
}
