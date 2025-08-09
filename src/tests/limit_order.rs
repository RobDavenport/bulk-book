#[cfg(test)]
use crate::{
    error::LimitOrderError,
    orderbook::{OrderBook, PriceLevel},
    types::{OrderId, Side},
};

// Testing Order Placement
#[test]
fn test_place_limit_bids() {
    let mut book = OrderBook::new();

    book.execute_limit_order(Side::Bid, OrderId(123), 100, 100)
        .unwrap();
    assert!(book.asks.is_empty());
    assert_eq!(book.bids.len(), 1);

    let order_index = book.index_map.get(&OrderId(123)).unwrap().order_index;
    assert_eq!(
        *book.bids.get(&100).unwrap(),
        PriceLevel {
            head: order_index,
            tail: order_index,
            order_count: 1
        }
    )
}

#[test]
fn test_place_limit_asks() {
    let mut book = OrderBook::new();

    book.execute_limit_order(Side::Ask, OrderId(123), 100, 100)
        .unwrap();
    assert!(book.bids.is_empty());
    assert_eq!(book.asks.len(), 1);

    let order_index = book.index_map.get(&OrderId(123)).unwrap().order_index;
    assert_eq!(
        *book.asks.get(&100).unwrap(),
        PriceLevel {
            head: order_index,
            tail: order_index,
            order_count: 1
        }
    )
}

#[test]
fn test_duplicate_order_id_errors() {
    let mut book = OrderBook::new();

    book.execute_limit_order(Side::Bid, OrderId(123), 100, 100)
        .unwrap();
    let duplicate = book.execute_limit_order(Side::Bid, OrderId(123), 222, 333);
    assert_eq!(duplicate, Err(LimitOrderError::OrderIdAlreadyExists));

    book.execute_limit_order(Side::Ask, OrderId(321), 100, 100)
        .unwrap();
    let duplicate = book.execute_limit_order(Side::Ask, OrderId(321), 222, 333);
    assert_eq!(duplicate, Err(LimitOrderError::OrderIdAlreadyExists));
}

#[test]
fn test_place_multiple_limit_bids_same_price() {
    let mut book = OrderBook::new();

    book.execute_limit_order(Side::Bid, OrderId(1), 100, 100)
        .unwrap();
    book.execute_limit_order(Side::Bid, OrderId(2), 100, 200)
        .unwrap();
    book.execute_limit_order(Side::Bid, OrderId(3), 100, 300)
        .unwrap();
    assert!(book.asks.is_empty());
    assert_eq!(book.bids.len(), 1);
    assert_eq!(book.bids.get(&100).unwrap().order_count, 3);

    let first = book.index_map.get(&OrderId(1)).unwrap().order_index;
    let third = book.index_map.get(&OrderId(3)).unwrap().order_index;

    assert_eq!(
        *book.bids.get(&100).unwrap(),
        PriceLevel {
            head: first,
            tail: third,
            order_count: 3
        }
    )
}

#[test]
fn test_place_multiple_limit_asks_same_price() {
    let mut book = OrderBook::new();

    book.execute_limit_order(Side::Ask, OrderId(1), 100, 100)
        .unwrap();
    book.execute_limit_order(Side::Ask, OrderId(2), 100, 200)
        .unwrap();
    book.execute_limit_order(Side::Ask, OrderId(3), 100, 300)
        .unwrap();
    assert!(book.bids.is_empty());
    assert_eq!(book.asks.len(), 1);
    assert_eq!(book.asks.get(&100).unwrap().order_count, 3);

    let first = book.index_map.get(&OrderId(1)).unwrap().order_index;
    let third = book.index_map.get(&OrderId(3)).unwrap().order_index;

    assert_eq!(
        *book.asks.get(&100).unwrap(),
        PriceLevel {
            head: first,
            tail: third,
            order_count: 3
        }
    )
}

#[test]
fn test_place_multiple_limit_bids_different_price() {
    let mut book = OrderBook::new();

    book.execute_limit_order(Side::Bid, OrderId(1), 100, 100)
        .unwrap();
    book.execute_limit_order(Side::Bid, OrderId(2), 200, 100)
        .unwrap();
    book.execute_limit_order(Side::Bid, OrderId(3), 300, 100)
        .unwrap();
    assert!(book.asks.is_empty());
    assert_eq!(book.bids.len(), 3);

    let first = book.index_map.get(&OrderId(1)).unwrap().order_index;
    let second = book.index_map.get(&OrderId(2)).unwrap().order_index;
    let third = book.index_map.get(&OrderId(3)).unwrap().order_index;

    assert_eq!(
        *book.bids.get(&100).unwrap(),
        PriceLevel {
            head: first,
            tail: first,
            order_count: 1
        }
    );
    assert_eq!(
        *book.bids.get(&200).unwrap(),
        PriceLevel {
            head: second,
            tail: second,
            order_count: 1
        }
    );
    assert_eq!(
        *book.bids.get(&300).unwrap(),
        PriceLevel {
            head: third,
            tail: third,
            order_count: 1
        }
    )
}

#[test]
fn test_place_multiple_limit_asks_different_price() {
    let mut book = OrderBook::new();

    book.execute_limit_order(Side::Ask, OrderId(1), 100, 100)
        .unwrap();
    book.execute_limit_order(Side::Ask, OrderId(2), 200, 100)
        .unwrap();
    book.execute_limit_order(Side::Ask, OrderId(3), 300, 100)
        .unwrap();
    assert!(book.bids.is_empty());
    assert_eq!(book.asks.len(), 3);

    let first = book.index_map.get(&OrderId(1)).unwrap().order_index;
    let second = book.index_map.get(&OrderId(2)).unwrap().order_index;
    let third = book.index_map.get(&OrderId(3)).unwrap().order_index;

    assert_eq!(
        *book.asks.get(&100).unwrap(),
        PriceLevel {
            head: first,
            tail: first,
            order_count: 1
        }
    );
    assert_eq!(
        *book.asks.get(&200).unwrap(),
        PriceLevel {
            head: second,
            tail: second,
            order_count: 1
        }
    );
    assert_eq!(
        *book.asks.get(&300).unwrap(),
        PriceLevel {
            head: third,
            tail: third,
            order_count: 1
        }
    )
}
