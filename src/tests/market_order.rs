#[cfg(test)]
use crate::{
    orderbook::{OrderBook, OrderNode, PriceLevel},
    types::{Fill, OrderId, Side},
};

#[test]
fn test_market_buy_greater_than_liquidity() {
    let mut book = OrderBook::new();

    book.execute_limit_order(Side::Ask, OrderId(1), 100, 1)
        .unwrap();

    let result = book.execute_market_order(Side::Bid, 2).unwrap();

    assert_eq!(result.len(), 1);
    assert_eq!(
        result[0],
        Fill {
            price: 100,
            quantity: 1
        }
    );

    // Limit Book should be completely empty
    assert_eq!(book.asks.len(), 0);
    assert_eq!(book.bids.len(), 0);
    assert_eq!(book.index_map.len(), 0);
    assert_eq!(book.orders.len(), 0);
}

#[test]
fn test_market_sell_greater_than_liquidity() {
    let mut book = OrderBook::new();

    book.execute_limit_order(Side::Bid, OrderId(1), 100, 1)
        .unwrap();

    let result = book.execute_market_order(Side::Ask, 2).unwrap();

    assert_eq!(result.len(), 1);
    assert_eq!(
        result[0],
        Fill {
            price: 100,
            quantity: 1
        }
    );

    // Limit Book should be completely empty
    assert_eq!(book.asks.len(), 0);
    assert_eq!(book.bids.len(), 0);
    assert_eq!(book.index_map.len(), 0);
    assert_eq!(book.orders.len(), 0);
}

#[test]
fn test_market_buy_no_liquidity() {
    let mut book = OrderBook::new();

    let result = book.execute_market_order(Side::Bid, 2).unwrap();

    assert_eq!(result.len(), 0);

    // Limit Book should be completely empty
    assert_eq!(book.asks.len(), 0);
    assert_eq!(book.bids.len(), 0);
    assert_eq!(book.index_map.len(), 0);
    assert_eq!(book.orders.len(), 0);
}

#[test]
fn test_market_sell_no_liquidity() {
    let mut book = OrderBook::new();

    let result = book.execute_market_order(Side::Ask, 2).unwrap();

    assert_eq!(result.len(), 0);

    // Limit Book should be completely empty
    assert_eq!(book.asks.len(), 0);
    assert_eq!(book.bids.len(), 0);
    assert_eq!(book.index_map.len(), 0);
    assert_eq!(book.orders.len(), 0);
}

#[test]
fn test_market_buy_less_than_liquidity() {
    let mut book = OrderBook::new();

    book.execute_limit_order(Side::Ask, OrderId(1), 100, 10)
        .unwrap();

    let result = book.execute_market_order(Side::Bid, 3).unwrap();

    assert_eq!(result.len(), 1);
    assert_eq!(
        result[0],
        Fill {
            price: 100,
            quantity: 3
        }
    );

    // Limit Book should sell have one level left
    assert_eq!(book.asks.len(), 1);
    assert_eq!(book.bids.len(), 0);
    assert_eq!(book.index_map.len(), 1);
    assert_eq!(book.orders.len(), 1);

    // Remaining level check
    let index = book.index_map.get(&OrderId(1)).unwrap().order_index;
    let node = book.orders.get(index).unwrap();
    assert_eq!(
        *node,
        OrderNode {
            quantity: 10 - 3,
            order_id: OrderId(1),
            previous: None,
            next: None
        }
    );
}

#[test]
fn test_market_sell_less_than_liquidity() {
    let mut book = OrderBook::new();

    book.execute_limit_order(Side::Bid, OrderId(1), 100, 10)
        .unwrap();

    let result = book.execute_market_order(Side::Ask, 3).unwrap();

    assert_eq!(result.len(), 1);
    assert_eq!(
        result[0],
        Fill {
            price: 100,
            quantity: 3
        }
    );

    // Limit Book should sell have one level left
    assert_eq!(book.asks.len(), 0);
    assert_eq!(book.bids.len(), 1);
    assert_eq!(book.index_map.len(), 1);
    assert_eq!(book.orders.len(), 1);

    // Remaining level check
    let index = book.index_map.get(&OrderId(1)).unwrap().order_index;
    let node = book.orders.get(index).unwrap();
    assert_eq!(
        *node,
        OrderNode {
            quantity: 10 - 3,
            order_id: OrderId(1),
            previous: None,
            next: None
        }
    );
}

#[test]
fn test_market_buy_complex_fills_same_price() {
    let mut book = OrderBook::new();

    book.execute_limit_order(Side::Ask, OrderId(1), 100, 1)
        .unwrap();
    book.execute_limit_order(Side::Ask, OrderId(2), 100, 2)
        .unwrap();
    book.execute_limit_order(Side::Ask, OrderId(3), 100, 3)
        .unwrap();
    assert!(book.bids.is_empty());
    assert_eq!(book.asks.len(), 1);

    // Get indices before they get removed
    let first = book.index_map.get(&OrderId(1)).unwrap().order_index;
    let second = book.index_map.get(&OrderId(2)).unwrap().order_index;
    let third = book.index_map.get(&OrderId(3)).unwrap().order_index;

    // Should have two fills
    let result = book.execute_market_order(Side::Bid, 2).unwrap();
    assert_eq!(result.len(), 2);
    assert_eq!(
        result[0],
        Fill {
            price: 100,
            quantity: 1
        }
    );
    assert_eq!(
        result[1],
        Fill {
            price: 100,
            quantity: 1
        }
    );

    // Check book is still correct
    let first_node = book.orders.get(first);
    let second_node = book.orders.get(second);
    let third_node = book.orders.get(third);

    assert_eq!(first_node, None);
    assert_eq!(
        second_node,
        Some(OrderNode {
            quantity: 1,
            order_id: OrderId(2),
            previous: None,
            next: Some(third)
        })
        .as_ref()
    );
    assert_eq!(
        third_node,
        Some(OrderNode {
            quantity: 3,
            order_id: OrderId(3),
            previous: Some(second),
            next: None
        })
        .as_ref()
    );
}

#[test]
fn test_market_sell_complex_fills_same_price() {
    let mut book = OrderBook::new();

    book.execute_limit_order(Side::Bid, OrderId(1), 100, 1)
        .unwrap();
    book.execute_limit_order(Side::Bid, OrderId(2), 100, 2)
        .unwrap();
    book.execute_limit_order(Side::Bid, OrderId(3), 100, 3)
        .unwrap();
    assert!(book.asks.is_empty());
    assert_eq!(book.bids.len(), 1);

    // Get indices before they get removed
    let first = book.index_map.get(&OrderId(1)).unwrap().order_index;
    let second = book.index_map.get(&OrderId(2)).unwrap().order_index;
    let third = book.index_map.get(&OrderId(3)).unwrap().order_index;

    // Should have two fills
    let result = book.execute_market_order(Side::Ask, 2).unwrap();
    assert_eq!(result.len(), 2);
    assert_eq!(
        result[0],
        Fill {
            price: 100,
            quantity: 1
        }
    );
    assert_eq!(
        result[1],
        Fill {
            price: 100,
            quantity: 1
        }
    );

    // Check book is still correct
    let first_node = book.orders.get(first);
    let second_node = book.orders.get(second);
    let third_node = book.orders.get(third);

    assert_eq!(first_node, None);
    assert_eq!(
        second_node,
        Some(OrderNode {
            quantity: 1,
            order_id: OrderId(2),
            previous: None,
            next: Some(third)
        })
        .as_ref()
    );
    assert_eq!(
        third_node,
        Some(OrderNode {
            quantity: 3,
            order_id: OrderId(3),
            previous: Some(second),
            next: None
        })
        .as_ref()
    );
}

#[test]
fn test_market_buy_complex_fills_different_price() {
    let mut book = OrderBook::new();

    book.execute_limit_order(Side::Ask, OrderId(1), 100, 1)
        .unwrap();
    book.execute_limit_order(Side::Ask, OrderId(2), 200, 2)
        .unwrap();
    book.execute_limit_order(Side::Ask, OrderId(3), 300, 3)
        .unwrap();
    assert!(book.bids.is_empty());
    assert_eq!(book.asks.len(), 3);

    // Get indices before they get removed
    let first = book.index_map.get(&OrderId(1)).unwrap().order_index;
    let second = book.index_map.get(&OrderId(2)).unwrap().order_index;
    let third = book.index_map.get(&OrderId(3)).unwrap().order_index;

    // Should have two fills
    let result = book.execute_market_order(Side::Bid, 2).unwrap();
    assert_eq!(result.len(), 2);
    assert_eq!(
        result[0],
        Fill {
            price: 100,
            quantity: 1
        }
    );
    assert_eq!(
        result[1],
        Fill {
            price: 200,
            quantity: 1
        }
    );

    // Check book is still correct
    let first_node = book.orders.get(first);
    let second_node = book.orders.get(second);
    let third_node = book.orders.get(third);

    assert_eq!(first_node, None);
    assert_eq!(
        second_node,
        Some(OrderNode {
            quantity: 1,
            order_id: OrderId(2),
            previous: None,
            next: None
        })
        .as_ref()
    );
    assert_eq!(
        third_node,
        Some(OrderNode {
            quantity: 3,
            order_id: OrderId(3),
            previous: None,
            next: None
        })
        .as_ref()
    );

    // Check Price Levels are still correct
    let first_price = book.asks.get(&100);
    let second_price = book.asks.get(&200);
    let third_price = book.asks.get(&300);

    assert_eq!(first_price, None);
    assert_eq!(
        second_price,
        Some(PriceLevel {
            head: second,
            tail: second,
            order_count: 1
        })
        .as_ref()
    );
    assert_eq!(
        third_price,
        Some(PriceLevel {
            head: third,
            tail: third,
            order_count: 1
        })
        .as_ref()
    );
}

#[test]
fn test_market_sell_complex_fills_different_price() {
    let mut book = OrderBook::new();

    book.execute_limit_order(Side::Bid, OrderId(1), 100, 2)
        .unwrap();
    book.execute_limit_order(Side::Bid, OrderId(2), 200, 2)
        .unwrap();
    book.execute_limit_order(Side::Bid, OrderId(3), 300, 3)
        .unwrap();
    assert!(book.asks.is_empty());
    assert_eq!(book.bids.len(), 3);

    // Get indices before they get removed
    let first = book.index_map.get(&OrderId(1)).unwrap().order_index;
    let second = book.index_map.get(&OrderId(2)).unwrap().order_index;
    let third = book.index_map.get(&OrderId(3)).unwrap().order_index;

    // Should have two fills
    let result = book.execute_market_order(Side::Ask, 4).unwrap();
    assert_eq!(result.len(), 2);
    assert_eq!(
        result[0],
        Fill {
            price: 300,
            quantity: 3
        }
    );
    assert_eq!(
        result[1],
        Fill {
            price: 200,
            quantity: 1
        }
    );

    // Check book is still correct
    let first_node = book.orders.get(first);
    let second_node = book.orders.get(second);
    let third_node = book.orders.get(third);

    assert_eq!(
        first_node,
        Some(OrderNode {
            quantity: 2,
            order_id: OrderId(1),
            previous: None,
            next: None
        })
        .as_ref()
    );
    assert_eq!(
        second_node,
        Some(OrderNode {
            quantity: 1,
            order_id: OrderId(2),
            previous: None,
            next: None
        })
        .as_ref()
    );
    assert_eq!(third_node, None);

    // Check Price Levels are still correct
    let first_price = book.bids.get(&100);
    let second_price = book.bids.get(&200);
    let third_price = book.bids.get(&300);

    assert_eq!(
        first_price,
        Some(PriceLevel {
            head: first,
            tail: first,
            order_count: 1
        })
        .as_ref()
    );
    assert_eq!(
        second_price,
        Some(PriceLevel {
            head: second,
            tail: second,
            order_count: 1
        })
        .as_ref()
    );
    assert_eq!(third_price, None);
}
