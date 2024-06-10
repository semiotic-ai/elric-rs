# elric-rs

This is a Rust implementation of the [substreams-sink-rust](https://github.com/streamingfast/substreams-sink-rust) with focus to insert data into Clickhouse as fast as possible.

The main difference between this and [sustreams-sink-sql](https://github.com/streamingfast/substreams-sink-sql) is about the clickhouse driver used. Our version uses [clickhouse.rs](https://github.com/loyd/clickhouse.rs) which contains some optimizations: instead of batching, we stream the data directly to clickhouse using async-inserts so the memory usage is way lower and the network load is spread throughout the program lifetime.

## Detail implementation

### Cursor Persistence

We use replace on duplicates to persist cursor. This means that we are constantly inserting the cursor and use the latest of them to recover from a stop.


### Block Undo Signal

We use the same strategy used by [substreams-sink-database](https://github.com/streamingfast/substreams-sink-sql) which we use a configurable buffer so we are up to chain head minus the buffer. This value is configured to be "final" so no undo blocks occours.
