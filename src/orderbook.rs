use std::collections::BTreeMap;

use hashbrown::HashMap;
use slab::Slab;

use crate::{
    error::{CancelOrderError, LimitOrderError, MarketOrderError},
    types::{OrderId, Price, Quantity, Side},
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OrderNode {
    pub quantity: Quantity,
    pub order_id: OrderId,
    pub previous: Option<usize>,
    pub next: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PriceLevel {
    pub head: usize,
    pub tail: usize,
    pub order_count: usize,
}

impl PriceLevel {
    pub fn head_mut<'a>(&self, memory: &'a mut Slab<OrderNode>) -> Option<&'a mut OrderNode> {
        memory.get_mut(self.head)
    }
}

pub struct OrderBook {
    bids: BTreeMap<Price, PriceLevel>,
    asks: BTreeMap<Price, PriceLevel>,
    orders: Slab<OrderNode>, // General Storage for order nodes
    index_map: HashMap<OrderId, IndexMapEntry>, // Reverse lookup Order Id, for fast cancels
}

pub struct IndexMapEntry {
    order_index: usize,
    price: Price,
    side: Side,
}

impl OrderBook {
    pub fn new() -> Self {
        Self {
            bids: Default::default(),
            asks: Default::default(),
            orders: Default::default(),
            index_map: Default::default(),
        }
    }

    pub fn cancel_order(&mut self, order_id: OrderId) -> Result<(), CancelOrderError> {
        // Lookup if order exists
        let Some(entry) = self.index_map.remove(&order_id) else {
            return Err(CancelOrderError::OrderIdNotFound);
        };
        let price_level_map = match entry.side {
            Side::Bid => &mut self.bids,
            Side::Ask => &mut self.asks,
        };

        // Find the price level
        let Some(price_level) = price_level_map.get_mut(&entry.price) else {
            return Err(CancelOrderError::InternalError);
        };
        let node_index = entry.order_index;

        // Store some local data to get around borrow checker
        let Some((quantity, prev_index, next_index)) = self
            .orders
            .get(node_index)
            .map(|node| (node.quantity, node.previous, node.next))
        else {
            return Err(CancelOrderError::InternalError);
        };

        // Update node indices
        if let Some(prev_node) = prev_index.and_then(|prev| self.orders.get_mut(prev)) {
            prev_node.next = next_index;
        } else {
            price_level.head = next_index.unwrap_or_default();
        }

        if let Some(next_node) = next_index.and_then(|next| self.orders.get_mut(next)) {
            next_node.previous = prev_index;
        } else {
            price_level.tail = prev_index.unwrap_or_default();
        }

        // Update meta-level things
        price_level.order_count -= 1;

        // Cleanup removed levels & order
        if price_level.order_count == 0 {
            price_level_map.remove(&entry.price);
        }

        self.orders.remove(node_index);

        Ok(())
    }

    fn next_bid(bids: &BTreeMap<Price, PriceLevel>) -> Option<(Price, PriceLevel)> {
        bids.first_key_value().map(|(k, v)| (k.clone(), v.clone()))
    }

    fn next_ask(asks: &BTreeMap<Price, PriceLevel>) -> Option<(Price, PriceLevel)> {
        asks.last_key_value().map(|(k, v)| (k.clone(), v.clone()))
    }

    fn next_bid_mut(bids: &mut BTreeMap<Price, PriceLevel>) -> Option<&mut PriceLevel> {
        bids.values_mut().last()
    }

    fn next_ask_mut(asks: &mut BTreeMap<Price, PriceLevel>) -> Option<&mut PriceLevel> {
        asks.values_mut().next()
    }

    pub fn execute_market_order(
        &mut self,
        side: Side,
        mut quantity: Quantity,
    ) -> Result<Vec<Fill>, MarketOrderError> {
        let mut fills = Vec::new();

        let (book, next, next_mut): (
            &mut BTreeMap<Price, PriceLevel>,
            fn(&BTreeMap<Price, PriceLevel>) -> Option<(Price, PriceLevel)>,
            fn(&mut BTreeMap<Price, PriceLevel>) -> Option<&mut PriceLevel>,
        ) = match side {
            Side::Bid => {
                let book = &mut self.asks;
                (book, Self::next_bid, Self::next_bid_mut)
            }
            Side::Ask => {
                let book = &mut self.bids;
                (book, Self::next_ask, Self::next_ask_mut)
            }
        };

        while quantity > 0 {
            let Some((price, mut top_level)) = next(book) else {
                break; // No more levels left in book
            };

            while let Some(node) = self.orders.get(top_level.head).cloned() {
                // This order will be fully consumed
                if quantity >= node.quantity {
                    fills.push(Fill {
                        price,
                        quantity: node.quantity,
                    });
                    quantity -= node.quantity;

                    // Remove the resting order from id lookup
                    self.index_map.remove(&node.order_id);

                    // Remove the resting order from the price level
                    self.orders.remove(top_level.head);
                    if let Some(next) = node.next {
                        // We need to update the pointer to the "next" order
                        let Some(top_level_ref) = next_mut(book) else {
                            return Err(MarketOrderError::InternalError);
                        };
                        if let Some(next_order) = self.orders.get_mut(next) {
                            next_order.previous = None;
                        }
                        top_level.head = next;
                        top_level_ref.head = next;
                    } else {
                        // No orders remain, just delete this level entirely
                        book.remove(&price);
                        break;
                    }
                } else {
                    // This resting order will be partially consumed
                    let Some(top_order_ref) = self.orders.get_mut(top_level.head) else {
                        return Err(MarketOrderError::InternalError);
                    };

                    // Push remaining quantity
                    fills.push(Fill {
                        price,
                        quantity: quantity,
                    });
                    top_order_ref.quantity -= quantity;
                    quantity = 0;
                    break;
                }
            }
        }

        Ok(fills)
    }

    pub fn execute_limit_order(
        &mut self,
        side: Side,
        order_id: OrderId,
        price: Price,
        quantity: Quantity,
    ) -> Result<(), LimitOrderError> {
        if self.index_map.get(&order_id).is_some() {
            return Err(LimitOrderError::OrderIdAlreadyExists);
        }

        let book = match side {
            Side::Bid => &mut self.bids,
            Side::Ask => &mut self.asks,
        };

        // Insert into memory
        let index = self.orders.insert(OrderNode {
            quantity,
            order_id,
            previous: None,
            next: None,
        });

        if let Some(level) = book.get_mut(&price) {
            // Link new order to previous tail
            let old_tail = level.tail;

            let Some(next) = self.orders.get_mut(old_tail) else {
                return Err(LimitOrderError::InternalError);
            };
            next.next = Some(index);

            let Some(previous) = self.orders.get_mut(index) else {
                return Err(LimitOrderError::InternalError);
            };
            previous.previous = Some(old_tail);

            // Update tail & order count
            level.tail = index;
            level.order_count += 1;
        } else {
            book.insert(
                price,
                PriceLevel {
                    head: index,
                    tail: index,
                    order_count: 1,
                },
            );
        }

        // Update the cancel map
        self.index_map.insert(
            order_id,
            IndexMapEntry {
                order_index: index,
                price,
                side,
            },
        );

        Ok(())
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct Fill {
    price: Price,
    quantity: Quantity,
}

#[cfg(test)]
mod tests {
    use crate::{
        error::LimitOrderError,
        orderbook::{Fill, OrderBook, OrderNode, PriceLevel},
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

    // Test Cancelation Logic
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
}
