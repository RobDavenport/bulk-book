# Overview

A minimal implementation of a CLOB in Rust. The following functions are implemented

- Place a resting Limit order
- Cancel a resting Limit order by Order ID
- Execute and match a Market Order

Matching is done by iterating through the best price levels in priority order (highest bid, lowest ask), then through orders in FIFO sequence at each price level, fully or partially filling incoming market orders until quantity is exhausted or book depth is depleted

Run tests via:

`cargo test`

Run benchmarks via:

`cargo bench`

# Design Overview

The core of the matching engine is two primary data structures. A linked-list for storing individual orders, and two BTree maps to store price levels, one each for bids and asks. The linked-list is simulated using `slab` for efficient index-based storage instead of messing with raw pointers.

In general, I think this approach has a great middle ground for various of market temperaments:
1. Slow, thick markets (Eurodollar Futures, Blue Chip Crypto) have very few price levels with many orders at the same level.
2. Fast, thin markets (Energy Futures, Altcoin Crypto) have many price levels with few orders at each level.
3. Standard markets which sit somewhere inbetween.

The linked-list approach for individual price levels was chosen for a few reasons:
- Easy appending to the end of the list when new orders are added
- Fast cancellation of orders at the front (or anywhere else) in the queue
- An extra OrderId based lookup map exists to reduce the need iterate through each price level to find the matching Ids. 
- This works well for the Slow & Thick Market Case

BTree Map was selected to store each price level linked-list for these reasons:
- Efficient insertion, lookup, and deletion of random price levels.
- Fast lookup for first and last prices (ie, best bid, and best ask)
- This works well for the Fast & Thin Market case

This combination of factors mean we can achieve:
- Constant time canceling of orders via OrderId lookup, without iterating the whole level.
- Efficient appending of new orders via fast Price Level lookups.
- Efficient order matching iterating through the top-of-book.

Of course there are a few downsides to this approach:
- It's quite complex due to state management and different kinds of lookups.
- Is still a generalized model, specific market conditions may favor other design decisions.
- May be more efficient with unsafe & raw pointers. Current approach copies data due to multiple borrow issues.

Other caveats:
- Prices and quantities are integer values
    - Additional overhead needed to translate between other services using multipliers or scaling methods.
- Limit Orders don't match when crossing the book.
    - This was done for two reasons:
        - The logic from market orders can be ported over or reused up-to the matching price
        - Some exchanges support RPI with crossed books for select makers
- OrderID usage is naive
    - Could be more efficient to generate this as part of the placement logic
    - But really depends on rest of stack for how these things should be managed

# Test Coverage

Tests are located in the `src/tests` module. Each inner file contains multiple tests for that specific aspect of the system. Test cases cover all of the listed functionalitiy, including error responses. Order matching tests include sets for each side (bids and asks), as their storage and iteration logic is different due to BTreeMap usage. Price-Time priority is ensured via tests, including complex situations like sweeping fills, mixed full and partial fills, and orders larger than the available liquidity.

# Benchmarks

Benchmarks are located in `benches/orderbook.rs` and use `criterion`. Four main categories exist:
- Order Insertion - Inserting to (empty/warm) orderbooks with (equal/separate) prices
- Order Matching - Test execution of (100/10,000) market orders
- Order Cancellation - Testing cancellation of 1000 orders across (equal/separate) prices
- "Trading Day" - Testing new book, insertion of orders, canceling of orders, and matching of orders in sequence.

# Benchmark Results Overview

Benchmarks were run on an Intel Core Ultra 7 laptop.

| Benchmark                                   | Median Time (µs) |
|--------------------------------------------|------------------|
| Limit Insert / insert_into_empty            | 408.7            |
| Limit Insert / insert_into_warm_book        | 79.5             |
| Limit Insert / insert_spread_into_empty     | 339.5            |
| Limit Insert / insert_spread_into_warm_book | 83.0             |
| Market Execution / match_100_orders_spread  | 2.14             |
| Market Execution / match_10_000_orders_spread| 193.5            |
| Order Cancel / cancel_sequential_in_large_book | 51.7           |
| Order Cancel / cancel_spread_in_large_book   | 51.6             |
| Stress / simulate_trading_session            | 47.0             |

> **Note:** These benchmark results should be interpreted with caution due to variability caused by other processes running concurrently on the test machine. Timing measurements fluctuate and may not reflect consistent performance. The full benchmark suite is included to allow reproducible testing under controlled conditions.

# Future Work

Some next steps to bring this to the next level and move towards a production grade system:
- Replication or Persistence Layer in case of outages.
- Stop Order Execution with flags for triggers
    - Mark Price, Last Trade Price, Bid/Ask based etc
- Allow limit orders to function as limit takers
- Additional Order Flags and Responses
    - Post Only, IOC, FOK, etc
- Additional benchmarks to cover more realistic scenarios
    - Staggered books, varying levels of liquidity across different prices
    - Different order sizes
