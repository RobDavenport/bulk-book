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
    pub head: Option<usize>,
    pub tail: Option<usize>,
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
            price_level.head = next_index;
        }

        if let Some(next_node) = next_index.and_then(|next| self.orders.get_mut(next)) {
            next_node.previous = prev_index;
        } else {
            price_level.tail = prev_index;
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
        quantity: Quantity,
    ) -> Vec<Fill> {
        // Determine side to match
        match side {
            Side::Bid => {
                // market buy -> match against asks
                let iter = self.asks.iter_mut();
                execute_market_order_with_iter(market_order_id, quantity, iter)
            }
            Side::Ask => {
                // market sell -> match against bids
                let iter = self.bids.iter_mut().rev();
                execute_market_order_with_iter(market_order_id, quantity, iter)
            }
        }
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

// TODO:
fn execute_market_order_with_iter<'a, I>(
    market_order_id: OrderId,
    mut remaining_qty: Quantity,
    book_iter: I,
) -> Vec<Fill>
where
    I: Iterator<Item = (&'a Price, &'a mut PriceLevel)>,
{
    let mut fills = Vec::new();

    for (price, level) in book_iter {
        // matching logic here
    }

    fills
}
