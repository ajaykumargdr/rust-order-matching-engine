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
