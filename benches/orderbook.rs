use std::hint::black_box;

use bulk_book::{
    orderbook::OrderBook,
    types::{OrderId, Price, Quantity, Side},
};
use criterion::{Criterion, criterion_group, criterion_main};

// Helper: generate sequential limit orders at same price
fn gen_orders(book: &mut OrderBook, side: Side, start_id: u64, count: usize, price: Price) {
    for i in 0..count {
        let order_id = OrderId(start_id + i as u64);
        book.execute_limit_order(side, order_id, price, 1).unwrap();
    }
}

// Helper: generate sequential limit orders at different prices
fn gen_orders_spread(
    book: &mut OrderBook,
    side: Side,
    start_id: u64,
    count: usize,
    price_start: Price,
    price_end: Price,
) {
    let price_range = price_end - price_start;
    for i in 0..count {
        let order_id = OrderId(start_id + i as u64);
        let price = price_start + (i as Price % price_range);
        book.execute_limit_order(side, order_id, price, 1).unwrap();
    }
}

// Benchmark 1: Limit Order Insert Performance
fn bench_limit_insert(c: &mut Criterion) {
    let mut group = c.benchmark_group("limit_insert");

    //single-price cold insert
    group.bench_function("insert_into_empty", |b| {
        b.iter(|| {
            let mut book = OrderBook::new();
            gen_orders(&mut book, Side::Bid, 0, 10_000, 100);
            black_box(book);
        });
    });

    // single-price warm insert
    group.bench_function("insert_into_warm_book", |b| {
        let mut initial_book = OrderBook::new();
        gen_orders(&mut initial_book, Side::Bid, 0, 10_000, 100);
        b.iter(|| {
            let mut book = initial_book.clone();
            gen_orders(&mut book, Side::Bid, 10_000, 1_000, 100);
            black_box(&book);
        });
    });

    // spread prices cold insert
    group.bench_function("insert_spread_into_empty", |b| {
        b.iter(|| {
            let mut book = OrderBook::new();
            gen_orders_spread(&mut book, Side::Bid, 0, 10_000, 90, 110);
            black_box(book);
        });
    });

    // spread prices warm insert
    group.bench_function("insert_spread_into_warm_book", |b| {
        let mut initial_book = OrderBook::new();
        gen_orders_spread(&mut initial_book, Side::Bid, 0, 10_000, 90, 110);
        b.iter(|| {
            let mut book = initial_book.clone();
            gen_orders_spread(&mut book, Side::Bid, 10_000, 1_000, 90, 110);
            black_box(&book);
        });
    });

    group.finish();
}

// Benchmark 2: Market Order Execution
fn bench_market_execution(c: &mut Criterion) {
    let mut group = c.benchmark_group("market_execution");

    group.bench_function("match_100_orders_spread", |b| {
        let mut initial_book = OrderBook::new();
        gen_orders_spread(&mut initial_book, Side::Ask, 0, 100, 95, 105);
        b.iter(|| {
            let mut book = initial_book.clone();
            let fills = book.execute_market_order(Side::Bid, 100).unwrap();
            black_box(&fills);
        });
    });

    group.bench_function("match_10_000_orders_spread", |b| {
        let mut initial_book = OrderBook::new();
        gen_orders_spread(&mut initial_book, Side::Ask, 0, 10_000, 95, 110);
        b.iter(|| {
            let mut book = initial_book.clone();
            let fills = book.execute_market_order(Side::Bid, 10_000).unwrap();
            black_box(&fills);
        });
    });

    group.finish();
}

// Benchmark 3: Order Cancel
fn bench_order_cancel(c: &mut Criterion) {
    let mut group = c.benchmark_group("order_cancel");

    const N: usize = 10_000;
    const COUNT: usize = 1000;
    const STEP: usize = 9967; // coprime to 10_000

    fn generate_unique_ids() -> [usize; COUNT] {
        let mut ids = [0; COUNT];
        let start = 0;
        for i in 0..COUNT {
            ids[i] = (start + i * STEP) % N;
        }
        ids
    }

    let unique_ids = generate_unique_ids();

    group.bench_function("cancel_sequential_in_large_book", |b| {
        let mut initial_book = OrderBook::new();
        gen_orders(&mut initial_book, Side::Bid, 0, 10_000, 100);

        b.iter(|| {
            let mut book = initial_book.clone();

            // Cancel a batch of orders per iteration deterministically
            for id in unique_ids {
                let result = book.cancel_order(OrderId(id as u64)).unwrap();
                black_box(result);
            }

            black_box(&book);
        });
    });

    group.bench_function("cancel_spread_in_large_book", |b| {
        let mut initial_book = OrderBook::new();
        gen_orders_spread(&mut initial_book, Side::Bid, 0, 10_000, 90, 110);

        b.iter(|| {
            let mut book = initial_book.clone();

            for id in unique_ids {
                let result = book.cancel_order(OrderId(id as u64)).unwrap();
                black_box(result);
            }

            black_box(&book);
        });
    });

    group.finish();
}

// Benchmark 4: Stress Scenario
fn bench_stress(c: &mut Criterion) {
    let mut group = c.benchmark_group("stress");

    // Pre-generate deterministic limit orders (side, price, order_id)
    const NUM_LIMIT_ORDERS: usize = 1000;
    let limit_orders: Vec<(Side, Price, OrderId)> = (0..NUM_LIMIT_ORDERS)
        .map(|i| {
            let side = if i % 2 == 0 { Side::Bid } else { Side::Ask };
            let price = 95 + (i as Price % 10); // prices from 95 to 104
            let order_id = OrderId(i as u64);
            (side, price, order_id)
        })
        .collect();

    // Pre-generate deterministic market orders (side, quantity)
    const NUM_MARKET_ORDERS: usize = 100;
    let market_orders: Vec<(Side, Quantity)> = (0..NUM_MARKET_ORDERS)
        .map(|i| {
            let side = if i % 2 == 0 { Side::Bid } else { Side::Ask };
            let qty = 1 + (i as Quantity % 50); // quantities 1 to 50
            (side, qty)
        })
        .collect();

    // Pre-generate deterministic cancels (cancel first 300 orders)
    const NUM_CANCEL_ORDERS: u64 = 300;
    let cancel_orders: Vec<OrderId> = (0..NUM_CANCEL_ORDERS).map(OrderId).collect();

    group.bench_function("simulate_trading_session", |b| {
        b.iter(|| {
            // Create fresh book for each iteration
            let mut book = OrderBook::new();

            // Insert all limit orders
            for &(side, price, order_id) in &limit_orders {
                black_box(book.execute_limit_order(side, order_id, price, 1).unwrap());
            }

            // Cancel subset of orders deterministically
            for &order_id in &cancel_orders {
                black_box(book.cancel_order(order_id).unwrap())
            }

            // Execute all market orders
            for &(side, qty) in &market_orders {
                black_box(book.execute_market_order(side, qty).unwrap());
            }

            black_box(&book);
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_limit_insert,
    bench_market_execution,
    bench_order_cancel,
    bench_stress
);
criterion_main!(benches);
