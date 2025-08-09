pub type Price = i64;
pub type Quantity = u64;

pub enum Side {
    Bid,
    Ask,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct OrderId(u64);

pub enum OrderType {
    Limit { price: Price },
    Market,
}
