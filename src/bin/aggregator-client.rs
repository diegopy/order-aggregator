use std::env;

use futures::StreamExt;
use orderbook::orderbook_aggregator_client::OrderbookAggregatorClient;

use crate::orderbook::Empty;

pub mod orderbook {
    tonic::include_proto!("orderbook");
}

#[tokio::main]
async fn main() {
    let mut args = env::args();
    if args.len() != 3 {
        eprintln!("Usage: aggregator-client <host> <port>");
        return;
    }
    args.next().unwrap();
    let host = args.next().unwrap();
    let port: u16 = args.next().unwrap().parse().expect("parsing port");
    let mut client = OrderbookAggregatorClient::connect(format!("http://{host}:{port}"))
        .await
        .expect("connecting");
    let response = client
        .book_summary(Empty::default())
        .await
        .expect("calling grpc endpoint");
    let mut response_stream = response.into_inner();
    while let Some(message) = response_stream.next().await {
        let summary = message.expect("unpacking message");
        println!("{:?}", summary);
    }
    println!("Exiting");
}
