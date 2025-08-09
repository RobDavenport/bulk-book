pub type Price = i64;
pub type Quantity = u64;

pub enum Side {
    Bid,
    Ask,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct OrderId(pub u64);

#[derive(Debug, PartialEq, Eq)]
pub struct Fill {
    pub price: Price,
    pub quantity: Quantity,
}
