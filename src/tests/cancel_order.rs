#[cfg(test)]
use crate::{
    orderbook::{OrderBook, OrderNode, PriceLevel},
    types::{OrderId, Side},
};

#[test]
fn test_cancel_rejection() {
    let mut book = OrderBook::new();
    let result = book.cancel_order(OrderId(1));
    assert_eq!(result, Err(crate::error::CancelOrderError::OrderIdNotFound));
}

#[test]
fn test_cancel_first_bid_of_three() {
    let mut book = OrderBook::new();

    book.execute_limit_order(Side::Bid, OrderId(1), 1, 1)
        .unwrap();
    book.execute_limit_order(Side::Bid, OrderId(2), 1, 2)
        .unwrap();
    book.execute_limit_order(Side::Bid, OrderId(3), 1, 3)
        .unwrap();
    assert!(book.asks.is_empty());
    assert_eq!(book.bids.len(), 1);

    // Get indices before they get removed
    let first = book.index_map.get(&OrderId(1)).unwrap().order_index;
    let second = book.index_map.get(&OrderId(2)).unwrap().order_index;
    let third = book.index_map.get(&OrderId(3)).unwrap().order_index;

    book.cancel_order(OrderId(1)).unwrap();

    // Check Nodes
    let first_node = book.orders.get(first);
    let second_node = book.orders.get(second);
    let third_node = book.orders.get(third);

    assert_eq!(first_node, None);
    assert_eq!(
        second_node,
        Some(OrderNode {
            quantity: 2,
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

    // Check Price Level
    let level = book.bids.get(&1).unwrap();
    assert_eq!(
        *level,
        PriceLevel {
            head: second,
            tail: third,
            order_count: 2
        }
    );
}

#[test]
fn test_cancel_second_bid_of_three() {
    let mut book = OrderBook::new();

    book.execute_limit_order(Side::Bid, OrderId(1), 1, 1)
        .unwrap();
    book.execute_limit_order(Side::Bid, OrderId(2), 1, 2)
        .unwrap();
    book.execute_limit_order(Side::Bid, OrderId(3), 1, 3)
        .unwrap();
    assert!(book.asks.is_empty());
    assert_eq!(book.bids.len(), 1);

    // Get indices before they get removed
    let first = book.index_map.get(&OrderId(1)).unwrap().order_index;
    let second = book.index_map.get(&OrderId(2)).unwrap().order_index;
    let third = book.index_map.get(&OrderId(3)).unwrap().order_index;

    book.cancel_order(OrderId(2)).unwrap();

    // Check Nodes
    let first_node = book.orders.get(first);
    let second_node = book.orders.get(second);
    let third_node = book.orders.get(third);

    assert_eq!(
        first_node,
        Some(OrderNode {
            quantity: 1,
            order_id: OrderId(1),
            previous: None,
            next: Some(third)
        })
        .as_ref()
    );
    assert_eq!(second_node, None);
    assert_eq!(
        third_node,
        Some(OrderNode {
            quantity: 3,
            order_id: OrderId(3),
            previous: Some(first),
            next: None
        })
        .as_ref()
    );

    // Check Price Level
    let level = book.bids.get(&1).unwrap();
    assert_eq!(
        *level,
        PriceLevel {
            head: first,
            tail: third,
            order_count: 2
        }
    );
}

#[test]
fn test_cancel_third_bid_of_three() {
    let mut book = OrderBook::new();

    book.execute_limit_order(Side::Bid, OrderId(1), 1, 1)
        .unwrap();
    book.execute_limit_order(Side::Bid, OrderId(2), 1, 2)
        .unwrap();
    book.execute_limit_order(Side::Bid, OrderId(3), 1, 3)
        .unwrap();
    assert!(book.asks.is_empty());
    assert_eq!(book.bids.len(), 1);

    // Get indices before they get removed
    let first = book.index_map.get(&OrderId(1)).unwrap().order_index;
    let second = book.index_map.get(&OrderId(2)).unwrap().order_index;
    let third = book.index_map.get(&OrderId(3)).unwrap().order_index;

    book.cancel_order(OrderId(3)).unwrap();

    // Check Nodes
    let first_node = book.orders.get(first);
    let second_node = book.orders.get(second);
    let third_node = book.orders.get(third);

    assert_eq!(
        first_node,
        Some(OrderNode {
            quantity: 1,
            order_id: OrderId(1),
            previous: None,
            next: Some(second)
        })
        .as_ref()
    );
    assert_eq!(
        second_node,
        Some(OrderNode {
            quantity: 2,
            order_id: OrderId(2),
            previous: Some(first),
            next: None
        })
        .as_ref()
    );
    assert_eq!(third_node, None);

    // Check Price Level
    let level = book.bids.get(&1).unwrap();
    assert_eq!(
        *level,
        PriceLevel {
            head: first,
            tail: second,
            order_count: 2
        }
    );
}

#[test]
fn test_cancel_first_ask_of_three() {
    let mut book = OrderBook::new();

    book.execute_limit_order(Side::Ask, OrderId(1), 1, 1)
        .unwrap();
    book.execute_limit_order(Side::Ask, OrderId(2), 1, 2)
        .unwrap();
    book.execute_limit_order(Side::Ask, OrderId(3), 1, 3)
        .unwrap();
    assert!(book.bids.is_empty());
    assert_eq!(book.asks.len(), 1);

    // Get indices before they get removed
    let first = book.index_map.get(&OrderId(1)).unwrap().order_index;
    let second = book.index_map.get(&OrderId(2)).unwrap().order_index;
    let third = book.index_map.get(&OrderId(3)).unwrap().order_index;

    book.cancel_order(OrderId(1)).unwrap();

    // Check Nodes
    let first_node = book.orders.get(first);
    let second_node = book.orders.get(second);
    let third_node = book.orders.get(third);

    assert_eq!(first_node, None);
    assert_eq!(
        second_node,
        Some(OrderNode {
            quantity: 2,
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

    // Check Price Level
    let level = book.asks.get(&1).unwrap();
    assert_eq!(
        *level,
        PriceLevel {
            head: second,
            tail: third,
            order_count: 2
        }
    );
}

#[test]
fn test_cancel_second_ask_of_three() {
    let mut book = OrderBook::new();

    book.execute_limit_order(Side::Ask, OrderId(1), 1, 1)
        .unwrap();
    book.execute_limit_order(Side::Ask, OrderId(2), 1, 2)
        .unwrap();
    book.execute_limit_order(Side::Ask, OrderId(3), 1, 3)
        .unwrap();
    assert!(book.bids.is_empty());
    assert_eq!(book.asks.len(), 1);

    // Get indices before they get removed
    let first = book.index_map.get(&OrderId(1)).unwrap().order_index;
    let second = book.index_map.get(&OrderId(2)).unwrap().order_index;
    let third = book.index_map.get(&OrderId(3)).unwrap().order_index;

    book.cancel_order(OrderId(2)).unwrap();

    // Check Nodes
    let first_node = book.orders.get(first);
    let second_node = book.orders.get(second);
    let third_node = book.orders.get(third);

    assert_eq!(
        first_node,
        Some(OrderNode {
            quantity: 1,
            order_id: OrderId(1),
            previous: None,
            next: Some(third)
        })
        .as_ref()
    );
    assert_eq!(second_node, None);
    assert_eq!(
        third_node,
        Some(OrderNode {
            quantity: 3,
            order_id: OrderId(3),
            previous: Some(first),
            next: None
        })
        .as_ref()
    );

    // Check Price Level
    let level = book.asks.get(&1).unwrap();
    assert_eq!(
        *level,
        PriceLevel {
            head: first,
            tail: third,
            order_count: 2
        }
    );
}

#[test]
fn test_cancel_third_ask_of_three() {
    let mut book = OrderBook::new();

    book.execute_limit_order(Side::Ask, OrderId(1), 1, 1)
        .unwrap();
    book.execute_limit_order(Side::Ask, OrderId(2), 1, 2)
        .unwrap();
    book.execute_limit_order(Side::Ask, OrderId(3), 1, 3)
        .unwrap();
    assert!(book.bids.is_empty());
    assert_eq!(book.asks.len(), 1);

    // Get indices before they get removed
    let first = book.index_map.get(&OrderId(1)).unwrap().order_index;
    let second = book.index_map.get(&OrderId(2)).unwrap().order_index;
    let third = book.index_map.get(&OrderId(3)).unwrap().order_index;

    book.cancel_order(OrderId(3)).unwrap();

    // Check Nodes
    let first_node = book.orders.get(first);
    let second_node = book.orders.get(second);
    let third_node = book.orders.get(third);

    assert_eq!(
        first_node,
        Some(OrderNode {
            quantity: 1,
            order_id: OrderId(1),
            previous: None,
            next: Some(second)
        })
        .as_ref()
    );
    assert_eq!(
        second_node,
        Some(OrderNode {
            quantity: 2,
            order_id: OrderId(2),
            previous: Some(first),
            next: None
        })
        .as_ref()
    );
    assert_eq!(third_node, None);

    // Check Price Level
    let level = book.asks.get(&1).unwrap();
    assert_eq!(
        *level,
        PriceLevel {
            head: first,
            tail: second,
            order_count: 2
        }
    );
}
