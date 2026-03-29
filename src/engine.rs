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

#[cfg(test)]
mod tests {
    use super::{OrderBook, process_order};
    use crate::models::{Order, Side};

    fn create_order(id: u64, side: Side, price: u64, qty: u64) -> Order {
        Order {
            id,
            side,
            price,
            qty,
        }
    }

    #[test]
    fn buy_order_no_asks() {
        let mut book = OrderBook::new();
        let order = create_order(0, Side::Buy, 100, 10);
        let result = process_order(order, &mut book);

        assert!(result.fills.is_empty());
        assert_eq!(result.deltas.len(), 1);
        assert_eq!(result.deltas[0].side, Side::Buy);
        assert_eq!(result.deltas[0].price, 100);
        assert_eq!(result.deltas[0].qty, 10);
        assert_eq!(book.bids.len(), 1);
        assert_eq!(book.bids.get(&100).unwrap().front().unwrap().qty, 10);
    }

    #[test]
    fn buy_order_full_match_single_ask() {
        let mut book = OrderBook::new();
        book.add_order(create_order(100, Side::Sell, 100, 10))
            .unwrap();

        let order = create_order(0, Side::Buy, 100, 10);
        let result = process_order(order, &mut book);

        assert_eq!(result.fills.len(), 1);
        assert_eq!(result.fills[0].maker_order_id, 100);
        assert_eq!(result.fills[0].taker_order_id, 0);
        assert_eq!(result.fills[0].price, 100);
        assert_eq!(result.fills[0].qty, 10);
        assert!(book.asks.is_empty());

        assert_eq!(result.deltas.len(), 1);
        assert_eq!(result.deltas[0].side, Side::Sell);
        assert_eq!(result.deltas[0].price, 100);
        assert_eq!(result.deltas[0].qty, 0);
    }

    #[test]
    fn buy_order_partial_match_single_ask() {
        let mut book = OrderBook::new();
        book.add_order(create_order(100, Side::Sell, 100, 20))
            .unwrap();

        let order = create_order(0, Side::Buy, 100, 10);
        let result = process_order(order, &mut book);

        assert_eq!(result.fills.len(), 1);
        assert_eq!(result.fills[0].qty, 10);
        assert_eq!(book.asks.len(), 1);
        assert_eq!(book.asks.get(&100).unwrap().front().unwrap().qty, 10);

        assert_eq!(result.deltas.len(), 1);
        assert_eq!(result.deltas[0].side, Side::Sell);
        assert_eq!(result.deltas[0].price, 100);
        assert_eq!(result.deltas[0].qty, 10);
    }

    #[test]
    fn buy_order_matches_multiple_asks_same_price() {
        let mut book = OrderBook::new();
        book.add_order(create_order(100, Side::Sell, 100, 5))
            .unwrap();
        book.add_order(create_order(101, Side::Sell, 100, 10))
            .unwrap();

        let order = create_order(1, Side::Buy, 100, 15);
        let result = process_order(order, &mut book);

        assert_eq!(result.fills.len(), 2);
        assert_eq!(result.fills[0].maker_order_id, 100);
        assert_eq!(result.fills[0].qty, 5);
        assert_eq!(result.fills[1].maker_order_id, 101);
        assert_eq!(result.fills[1].qty, 10);
        assert!(book.asks.is_empty());

        assert_eq!(result.deltas.len(), 1);
        assert_eq!(result.deltas[0].side, Side::Sell);
        assert_eq!(result.deltas[0].price, 100);
        assert_eq!(result.deltas[0].qty, 0);
    }

    #[test]
    fn buy_order_matchers_multiple_asks_different_prices() {
        let mut book = OrderBook::new();
        book.add_order(create_order(100, Side::Sell, 95, 5))
            .unwrap();
        book.add_order(create_order(101, Side::Sell, 100, 10))
            .unwrap();

        let order = create_order(1, Side::Buy, 100, 15);
        let result = process_order(order, &mut book);

        assert_eq!(result.fills.len(), 2);
        assert_eq!(result.fills[0].maker_order_id, 100);
        assert_eq!(result.fills[0].price, 95);
        assert_eq!(result.fills[0].qty, 5);
        assert_eq!(result.fills[1].maker_order_id, 101);
        assert_eq!(result.fills[1].price, 100);
        assert_eq!(result.fills[1].qty, 10);
        assert!(book.asks.is_empty());

        assert_eq!(result.deltas.len(), 2);
        assert!(
            result
                .deltas
                .iter()
                .any(|d| d.side == Side::Sell && d.price == 95 && d.qty == 0)
        );
        assert!(
            result
                .deltas
                .iter()
                .any(|d| d.side == Side::Sell && d.price == 100 && d.qty == 0)
        );
    }

    #[test]
    fn buy_order_no_match_price_too_low() {
        let mut book = OrderBook::new();
        book.add_order(create_order(100, Side::Sell, 100, 10))
            .unwrap();

        let order = create_order(1, Side::Buy, 90, 10);
        let result = process_order(order, &mut book);

        assert!(result.fills.is_empty());
        assert_eq!(book.asks.len(), 1);
        assert_eq!(book.bids.len(), 1);
        assert_eq!(book.bids.get(&90).unwrap().front().unwrap().qty, 10);

        assert_eq!(result.deltas.len(), 1);
        assert_eq!(result.deltas[0].side, Side::Buy);
        assert_eq!(result.deltas[0].price, 90);
        assert_eq!(result.deltas[0].qty, 10);
    }

    #[test]
    fn buy_order_better_price_gets_better_fill() {
        let mut book = OrderBook::new();
        book.add_order(create_order(100, Side::Sell, 100, 10))
            .unwrap();

        let order = create_order(1, Side::Buy, 110, 10);
        let result = process_order(order, &mut book);

        assert_eq!(result.fills.len(), 1);
        assert_eq!(result.fills[0].price, 100);
        assert!(book.asks.is_empty());

        assert_eq!(result.deltas.len(), 1);
        assert_eq!(result.deltas[0].side, Side::Sell);
        assert_eq!(result.deltas[0].price, 100);
        assert_eq!(result.deltas[0].qty, 0);
    }

    #[test]
    fn buy_order_partial_then_rests() {
        let mut book = OrderBook::new();
        book.add_order(create_order(100, Side::Sell, 100, 5))
            .unwrap();

        let order = create_order(1, Side::Buy, 100, 10);
        let result = process_order(order, &mut book);

        assert_eq!(result.fills.len(), 1);
        assert_eq!(result.fills[0].qty, 5);
        assert_eq!(book.bids.len(), 1);
        assert_eq!(book.bids.get(&100).unwrap().front().unwrap().qty, 5);

        assert_eq!(result.deltas.len(), 2);
        assert!(
            result
                .deltas
                .iter()
                .any(|d| d.side == Side::Sell && d.price == 100 && d.qty == 0)
        );
        assert!(
            result
                .deltas
                .iter()
                .any(|d| d.side == Side::Buy && d.price == 100 && d.qty == 5)
        );
    }

    #[test]
    fn sell_order_no_bids() {
        let mut book = OrderBook::new();
        let order = create_order(1, Side::Sell, 100, 10);
        let result = process_order(order, &mut book);

        assert!(result.fills.is_empty());
        assert_eq!(result.deltas.len(), 1);
        assert_eq!(result.deltas[0].side, Side::Sell);
        assert_eq!(result.deltas[0].price, 100);
        assert_eq!(result.deltas[0].qty, 10);
        assert_eq!(book.asks.len(), 1);
        assert_eq!(book.asks.get(&100).unwrap().front().unwrap().qty, 10);
    }

    #[test]
    fn sell_order_full_match_single_bid() {
        let mut book = OrderBook::new();
        book.add_order(create_order(100, Side::Buy, 100, 10))
            .unwrap();

        let order = create_order(1, Side::Sell, 100, 10);
        let result = process_order(order, &mut book);

        assert_eq!(result.fills.len(), 1);
        assert_eq!(result.fills[0].maker_order_id, 100);
        assert_eq!(result.fills[0].taker_order_id, 1);
        assert_eq!(result.fills[0].price, 100);
        assert_eq!(result.fills[0].qty, 10);
        assert!(book.bids.is_empty());

        assert_eq!(result.deltas.len(), 1);
        assert_eq!(result.deltas[0].side, Side::Buy);
        assert_eq!(result.deltas[0].price, 100);
        assert_eq!(result.deltas[0].qty, 0);
    }

    #[test]
    fn sell_order_partial_match_single_bid() {
        let mut book = OrderBook::new();
        book.add_order(create_order(100, Side::Buy, 100, 20))
            .unwrap();

        let order = create_order(1, Side::Sell, 100, 10);
        let result = process_order(order, &mut book);

        assert_eq!(result.fills.len(), 1);
        assert_eq!(result.fills[0].qty, 10);
        assert_eq!(book.bids.len(), 1);
        assert_eq!(book.bids.get(&100).unwrap().front().unwrap().qty, 10);

        assert_eq!(result.deltas.len(), 1);
        assert_eq!(result.deltas[0].side, Side::Buy);
        assert_eq!(result.deltas[0].price, 100);
        assert_eq!(result.deltas[0].qty, 10);
    }

    #[test]
    fn sell_order_multiple_bids_same_price() {
        let mut book = OrderBook::new();
        book.add_order(create_order(100, Side::Buy, 100, 5))
            .unwrap();
        book.add_order(create_order(101, Side::Buy, 100, 10))
            .unwrap();

        let order = create_order(1, Side::Sell, 100, 15);
        let result = process_order(order, &mut book);

        assert_eq!(result.fills.len(), 2);
        assert_eq!(result.fills[0].maker_order_id, 100);
        assert_eq!(result.fills[0].qty, 5);
        assert_eq!(result.fills[1].maker_order_id, 101);
        assert_eq!(result.fills[1].qty, 10);
        assert!(book.bids.is_empty());

        assert_eq!(result.deltas.len(), 1);
        assert_eq!(result.deltas[0].side, Side::Buy);
        assert_eq!(result.deltas[0].price, 100);
        assert_eq!(result.deltas[0].qty, 0);
    }

    #[test]
    fn sell_order_multiple_bids_different_prices() {
        let mut book = OrderBook::new();
        book.add_order(create_order(100, Side::Buy, 100, 5))
            .unwrap();
        book.add_order(create_order(101, Side::Buy, 105, 10))
            .unwrap();

        let order = create_order(1, Side::Sell, 100, 15);
        let result = process_order(order, &mut book);

        assert_eq!(result.fills.len(), 2);
        assert_eq!(result.fills[0].maker_order_id, 101);
        assert_eq!(result.fills[0].price, 105);
        assert_eq!(result.fills[0].qty, 10);
        assert_eq!(result.fills[1].maker_order_id, 100);
        assert_eq!(result.fills[1].price, 100);
        assert_eq!(result.fills[1].qty, 5);
        assert!(book.bids.is_empty());

        assert_eq!(result.deltas.len(), 2);
        assert!(
            result
                .deltas
                .iter()
                .any(|d| d.side == Side::Buy && d.price == 100 && d.qty == 0)
        );
        assert!(
            result
                .deltas
                .iter()
                .any(|d| d.side == Side::Buy && d.price == 105 && d.qty == 0)
        );
    }

    #[test]
    fn sell_order_no_match_price_too_high() {
        let mut book = OrderBook::new();
        book.add_order(create_order(100, Side::Buy, 100, 10))
            .unwrap();

        let order = create_order(1, Side::Sell, 110, 10);
        let result = process_order(order, &mut book);

        assert!(result.fills.is_empty());
        assert_eq!(book.bids.len(), 1);
        assert_eq!(book.asks.len(), 1);
        assert_eq!(book.asks.get(&110).unwrap().front().unwrap().qty, 10);

        assert_eq!(result.deltas.len(), 1);
        assert_eq!(result.deltas[0].side, Side::Sell);
        assert_eq!(result.deltas[0].price, 110);
        assert_eq!(result.deltas[0].qty, 10);
    }

    #[test]
    fn sell_order_lower_price_gets_better_fill() {
        let mut book = OrderBook::new();
        book.add_order(create_order(100, Side::Buy, 100, 10))
            .unwrap();

        let order = create_order(1, Side::Sell, 90, 10);
        let result = process_order(order, &mut book);

        assert_eq!(result.fills.len(), 1);
        assert_eq!(result.fills[0].price, 100);
        assert!(book.bids.is_empty());

        assert_eq!(result.deltas.len(), 1);
        assert_eq!(result.deltas[0].side, Side::Buy);
        assert_eq!(result.deltas[0].price, 100);
        assert_eq!(result.deltas[0].qty, 0);
    }

    #[test]
    fn zero_quantity_order() {
        let mut book = OrderBook::new();
        book.add_order(create_order(100, Side::Sell, 100, 10))
            .unwrap();

        let order = create_order(1, Side::Buy, 100, 0);
        let result = process_order(order, &mut book);

        assert!(result.fills.is_empty());
        assert_eq!(book.asks.len(), 1);
        assert!(book.bids.is_empty());
    }

    #[test]
    fn crossed_book_sell() {
        let mut book = OrderBook::new();
        book.add_order(create_order(100, Side::Buy, 105, 10))
            .unwrap();

        let order = create_order(1, Side::Sell, 100, 10);
        let result = process_order(order, &mut book);

        assert_eq!(result.fills.len(), 1);
        assert_eq!(result.fills[0].price, 105);
        assert!(book.bids.is_empty());

        assert_eq!(result.deltas.len(), 1);
        assert_eq!(result.deltas[0].side, Side::Buy);
        assert_eq!(result.deltas[0].price, 105);
        assert_eq!(result.deltas[0].qty, 0);
    }
}
