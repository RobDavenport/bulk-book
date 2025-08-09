#[derive(Debug, PartialEq, Eq)]
pub enum CancelOrderError {
    OrderIdNotFound,
    InternalError,
}

#[derive(Debug, PartialEq, Eq)]
pub enum MarketOrderError {
    InternalError,
}

#[derive(Debug, PartialEq, Eq)]
pub enum LimitOrderError {
    OrderIdAlreadyExists,
    InternalError,
}
