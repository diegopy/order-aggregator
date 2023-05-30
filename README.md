# order-aggregator
## Setup
1. Install [Rust](https://rustup.rs/).
2. Install [Tonic dependencies](https://github.com/hyperium/tonic#getting-started).

## Starting up
### Configuration
A sample configuration file is provided in config.toml. Config is read from the file and can be overriden via environment variables. A .env file is provided as an example, overriding the server port. The server loads the variables in the .env file at startup.

### Server
Start the server with `cargo run --bin order-aggregator`

### Client
A very simple command line client for testing is also provided. To start it up and connect to the default server configuration, use `cargo run --bin aggregator-client [::1] 5000`. It will print on stdout every message received from the streaming GRPC endpoint.

## Notes
- The `aggregator` module was made with extensibility in mind. It supports any number of exchange sources, doing the ask/bid sorting and merging using merging with a heap data structure strategy.
- The default is to trust that the exchanges API will return their ask/bids already sorted. If desired, each exchange configuration has a `sort` option that can be set to true to do sorting before submitting to aggregation.
