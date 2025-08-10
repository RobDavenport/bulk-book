use std::collections::BTreeMap;

use hashbrown::HashMap;
use slab::Slab;

use crate::{
    error::{CancelOrderError, LimitOrderError, MarketOrderError},
    types::{Fill, OrderId, Price, Quantity, Side},
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

type BookSideType = BTreeMap<Price, PriceLevel>;

#[derive(Debug, Clone)]
pub struct OrderBook {
    pub bids: BookSideType,
    pub asks: BookSideType,
    pub orders: Slab<OrderNode>, // General Storage for order nodes
    pub index_map: HashMap<OrderId, IndexMapEntry>, // Reverse lookup Order Id, for fast cancels
}

impl Default for OrderBook {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone)]
pub struct IndexMapEntry {
    pub order_index: usize,
    pub price: Price,
    pub side: Side,
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
        let Some((prev_index, next_index)) = self
            .orders
            .get(node_index)
            .map(|node| (node.previous, node.next))
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

    fn next_bid(bids: &BookSideType) -> Option<(Price, PriceLevel)> {
        bids.last_key_value().map(|(k, v)| (*k, v.clone()))
    }

    fn next_ask(asks: &BookSideType) -> Option<(Price, PriceLevel)> {
        asks.first_key_value().map(|(k, v)| (*k, v.clone()))
    }

    fn next_bid_mut(bids: &mut BookSideType) -> Option<&mut PriceLevel> {
        bids.values_mut().last()
    }

    fn next_ask_mut(asks: &mut BookSideType) -> Option<&mut PriceLevel> {
        asks.values_mut().next()
    }

    pub fn execute_market_order(
        &mut self,
        side: Side,
        mut quantity: Quantity,
    ) -> Result<Vec<Fill>, MarketOrderError> {
        struct MarketOrderHelper<'a> {
            book: &'a mut BookSideType,
            next_fn: fn(&BookSideType) -> Option<(Price, PriceLevel)>,
            next_mut_fn: fn(&mut BookSideType) -> Option<&mut PriceLevel>,
        }

        let MarketOrderHelper {
            book,
            next_fn,
            next_mut_fn,
        } = match side {
            Side::Bid => {
                let book = &mut self.asks;
                MarketOrderHelper {
                    book,
                    next_fn: Self::next_ask,
                    next_mut_fn: Self::next_ask_mut,
                }
            }
            Side::Ask => {
                let book = &mut self.bids;
                MarketOrderHelper {
                    book,
                    next_fn: Self::next_bid,
                    next_mut_fn: Self::next_bid_mut,
                }
            }
        };

        let mut fills = Vec::new();

        while quantity > 0 {
            let Some((price, mut top_level)) = next_fn(book) else {
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

                    // Remove the node from memory
                    self.orders.remove(top_level.head);

                    // Remove the resting order from the price level
                    if let Some(next) = node.next {
                        // We need to update the pointer to the "next" order
                        let Some(top_level_ref) = next_mut_fn(book) else {
                            return Err(MarketOrderError::InternalError);
                        };
                        if let Some(next_order) = self.orders.get_mut(next) {
                            next_order.previous = None;
                        }
                        top_level.head = next;
                        top_level.order_count -= 1;

                        // Sync the local and stored values.
                        *top_level_ref = top_level.clone();
                    } else {
                        // No orders remain, just delete this level entirely
                        book.remove(&price);
                        break;
                    }
                } else {
                    // This resting order will be partially consumed
                    let Some(top_node_ref) = self.orders.get_mut(top_level.head) else {
                        return Err(MarketOrderError::InternalError);
                    };

                    // Push remaining quantity
                    fills.push(Fill { price, quantity });
                    top_node_ref.quantity -= quantity;
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
