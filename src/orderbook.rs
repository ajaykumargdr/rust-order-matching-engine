use std::collections::{BTreeMap, VecDeque};

use crate::models::{Order, Side};

#[derive(Debug)]
pub struct OrderBook {
    pub bids: BTreeMap<u64, VecDeque<Order>>,
    pub asks: BTreeMap<u64, VecDeque<Order>>,
}

impl OrderBook {
    pub fn new() -> Self {
        Self {
            bids: BTreeMap::new(),
            asks: BTreeMap::new(),
        }
    }

    pub fn add_order(&mut self, order: Order) -> Result<(), String> {
        if order.qty == 0 {
            return Err("Zero quantity order".to_string());
        }

        if order.price == 0 {
            return Err("Zero price order".to_string());
        }

        match order.side {
            Side::Buy => {
                self.bids.entry(order.price).or_default().push_back(order);
            }
            Side::Sell => {
                self.asks.entry(order.price).or_default().push_back(order);
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{Order, OrderBook, Side};

    fn create_order(id: u64, side: Side, price: u64, qty: u64) -> Order {
        Order {
            id,
            side,
            price,
            qty,
        }
    }

    #[test]
    fn add_single_bid_order() {
        let mut ob = OrderBook::new();
        let order = create_order(1, Side::Buy, 100, 10);
        ob.add_order(order).unwrap();

        assert_eq!(ob.bids.len(), 1);
        assert!(ob.asks.is_empty());

        let level = ob.bids.get(&100).unwrap();
        assert_eq!(level.len(), 1);
        assert_eq!(level.front().unwrap().id, 1);
    }

    #[test]
    fn add_single_ask_order() {
        let mut ob = OrderBook::new();
        let order = create_order(1, Side::Sell, 200, 5);
        ob.add_order(order).unwrap();

        assert_eq!(ob.asks.len(), 1);
        assert!(ob.bids.is_empty());

        let level = ob.asks.get(&200).unwrap();
        assert_eq!(level.len(), 1);
        assert_eq!(level.front().unwrap().id, 1);
    }

    #[test]
    fn add_multiple_bids_same_price() {
        let mut ob = OrderBook::new();
        ob.add_order(create_order(1, Side::Buy, 100, 10)).unwrap();
        ob.add_order(create_order(2, Side::Buy, 100, 20)).unwrap();

        let level = ob.bids.get(&100).unwrap();
        assert_eq!(level.len(), 2);
        assert_eq!(level.front().unwrap().id, 1);
        assert_eq!(level.back().unwrap().id, 2);
    }

    #[test]
    fn add_multiple_bids_different_prices() {
        let mut ob = OrderBook::new();
        ob.add_order(create_order(1, Side::Buy, 100, 10)).unwrap();
        ob.add_order(create_order(2, Side::Buy, 95, 20)).unwrap();
        ob.add_order(create_order(3, Side::Buy, 105, 15)).unwrap();

        assert_eq!(ob.bids.len(), 3);
        assert!(ob.bids.contains_key(&95));
        assert!(ob.bids.contains_key(&100));
        assert!(ob.bids.contains_key(&105));
    }

    #[test]
    fn add_multiple_asks_same_price() {
        let mut ob = OrderBook::new();
        ob.add_order(create_order(1, Side::Sell, 200, 10)).unwrap();
        ob.add_order(create_order(2, Side::Sell, 200, 20)).unwrap();

        let level = ob.asks.get(&200).unwrap();
        assert_eq!(level.len(), 2);
        assert_eq!(level.front().unwrap().id, 1);
        assert_eq!(level.back().unwrap().id, 2);
    }

    #[test]
    fn add_multiple_asks_different_prices() {
        let mut ob = OrderBook::new();
        ob.add_order(create_order(1, Side::Sell, 200, 10)).unwrap();
        ob.add_order(create_order(2, Side::Sell, 205, 20)).unwrap();
        ob.add_order(create_order(3, Side::Sell, 195, 15)).unwrap();

        assert_eq!(ob.asks.len(), 3);
        assert!(ob.asks.contains_key(&195));
        assert!(ob.asks.contains_key(&200));
        assert!(ob.asks.contains_key(&205));
    }

    #[test]
    fn add_mixed_orders() {
        let mut ob = OrderBook::new();
        ob.add_order(create_order(1, Side::Buy, 100, 10)).unwrap();
        ob.add_order(create_order(2, Side::Sell, 110, 5)).unwrap();
        ob.add_order(create_order(3, Side::Buy, 99, 20)).unwrap();
        ob.add_order(create_order(4, Side::Sell, 109, 15)).unwrap();

        assert_eq!(ob.bids.len(), 2);
        assert_eq!(ob.asks.len(), 2);
        assert!(ob.bids.contains_key(&100));
        assert!(ob.bids.contains_key(&99));
        assert!(ob.asks.contains_key(&110));
        assert!(ob.asks.contains_key(&109));
    }

    #[test]
    fn order_sequence() {
        let mut ob = OrderBook::new();
        ob.add_order(create_order(1, Side::Sell, 300, 10)).unwrap();
        ob.add_order(create_order(2, Side::Sell, 100, 10)).unwrap();
        ob.add_order(create_order(3, Side::Sell, 200, 10)).unwrap();

        let ask_prices: Vec<u64> = ob.asks.keys().cloned().collect();
        assert_eq!(ask_prices, vec![100, 200, 300]);

        ob.add_order(create_order(4, Side::Buy, 150, 10)).unwrap();
        ob.add_order(create_order(5, Side::Buy, 50, 10)).unwrap();
        ob.add_order(create_order(6, Side::Buy, 250, 10)).unwrap();

        let bid_prices: Vec<u64> = ob.bids.keys().cloned().collect();
        assert_eq!(bid_prices, vec![50, 150, 250]);
    }

    #[test]
    fn zero_quantity_order() {
        let mut ob = OrderBook::new();
        let order = create_order(1, Side::Buy, 100, 0);
        assert_eq!(ob.add_order(order), Err("Zero quantity order".to_string()));
    }

    #[test]
    fn zero_price_order() {
        let mut ob = OrderBook::new();
        let order = create_order(1, Side::Buy, 0, 10);
        assert_eq!(ob.add_order(order), Err("Zero price order".to_string()));
    }
}
