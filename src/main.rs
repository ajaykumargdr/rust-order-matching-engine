mod engine;
mod models;
mod orderbook;

use models::{Order, Side};
use orderbook::OrderBook;

fn main() {
    let mut book = OrderBook::new();

    let sell_order = Order {
        id: 1,
        side: Side::Sell,
        price: 90,
        qty: 5,
    };

    engine::process_order(sell_order, &mut book);

    let buy_order = Order {
        id: 2,
        side: Side::Buy,
        price: 100,
        qty: 10,
    };

    let fills = engine::process_order(buy_order, &mut book);

    println!("Fills: {:?}", fills);
    println!("Remaining OrderBook: {:?}", book);
}
