use std::collections::BTreeMap;

use hashbrown::HashMap;
use slab::Slab;

use crate::{
    error::CancelOrderError,
    types::{OrderId, Price, Quantity, Side},
};

pub struct OrderNode {
    pub quantity: Quantity,
    pub order_id: OrderId,
    pub previous: Option<usize>,
    pub next: Option<usize>,
}

pub struct Order {
    pub id: OrderId,
    pub side: Side,
    pub remaining: Quantity,
    pub timestamp: u128,
}

pub struct PriceLevel {
    pub head: usize,
    pub tail: usize,
    pub total_quantity: Quantity,
    pub order_count: usize,
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
        price_level.total_quantity -= quantity;
        price_level.order_count -= 1;

        // Cleanup removed levels & order
        if price_level.order_count == 0 {
            price_level_map.remove(&entry.price);
        }

        self.orders.remove(node_index);

        Ok(())
    }

    pub fn execute_market_order(
        &mut self,
        market_order_id: OrderId,
        side: Side,
        mut quantity: Quantity,
    ) -> Vec<Fill> {
        let mut fills = Vec::new();

        let book = match side {
            Side::Bid => &mut self.asks, // market buy -> match against asks
            Side::Ask => &mut self.bids, // market sell -> match against bids
        };

        let mut level_match = |mut remaining_quantity: Quantity, (price, level): (&Price, &mut PriceLevel)| -> Quantity {
            // Order will fully consume this level
            if remaining_quantity >= level.total_quantity {
                remaining_quantity -= level.total_quantity;

                let mut iter = PriceLevelIter::new(level);
                while let Some((index, node)) = iter.next(&mut self.orders) {
                    self.index_map.remove(&node.order_id);
                    self.orders.remove(index);
                }
            }

            remaining_quantity
        };

        // match side {
        //     Side::Bid => {
        //         for item in self.asks.iter_mut() {
        //             if quantity == 0 {
        //                 break;
        //             }

        //             quantity = level_match(quantity, item);
        //         }
        //     }
        //     Side::Ask => {
        //         for item in self.bids.iter_mut().rev() {
        //             if quantity == 0 {
        //                 break;
        //             }

        //             quantity = level_match(quantity, item);
        //         }
        //     }
        // }

        fills
    }

    pub fn execute_limit_order(
        &mut self,
        limit_order_id: OrderId,
        price: Price,
        quantity: Quantity,
    ) {
        //TODO:
    }
}

// TODO:
pub struct Fill;

struct PriceLevelIter {
    index: Option<usize>,
}

impl PriceLevelIter {
    pub fn new(price_level: &PriceLevel) -> Self {
        Self { index: Some(price_level.head) }
    }

    pub fn next<'a>(&'a mut self, memory: &'a mut Slab<OrderNode>) -> Option<(usize, &'a mut OrderNode)> {
        if let Some((index, node)) = self.index.and_then(|i| memory.get_mut(i).map(|n| (i, n))) {
            let current_index = index;
            self.index = node.next;
            Some((current_index, node))
        } else {
            None
        }
    }
}
