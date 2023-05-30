use std::net::ToSocketAddrs;
use std::pin::Pin;

use crate::{configuration, orderbook};
use async_trait::async_trait;
use futures::{stream, Stream, StreamExt};
use tokio::sync::watch::Receiver;
use tokio_util::sync::CancellationToken;
use tonic::transport::Server;
use tonic::Status;

pub async fn grpc_server(
    ticks: Receiver<orderbook::Summary>,
    stop_signal: CancellationToken,
    server_config: configuration::Server,
) -> anyhow::Result<()> {
    let server = OrderBookAggregatorService { ticks };
    let port = server_config.port;
    Server::builder()
        .add_service(orderbook::orderbook_aggregator_server::OrderbookAggregatorServer::new(server))
        .serve_with_shutdown(
            format!("[::1]:{port}")
                .to_socket_addrs()
                .unwrap()
                .next()
                .unwrap(),
            stop_signal.cancelled(),
        )
        .await?;

    Ok(())
}

struct OrderBookAggregatorService {
    pub ticks: Receiver<orderbook::Summary>,
}

type BookSummaryResponseStream =
    Pin<Box<dyn Stream<Item = Result<orderbook::Summary, Status>> + Send>>;

#[async_trait]
impl orderbook::orderbook_aggregator_server::OrderbookAggregator for OrderBookAggregatorService {
    type BookSummaryStream = BookSummaryResponseStream;

    async fn book_summary(
        &self,
        _request: tonic::Request<orderbook::Empty>,
    ) -> Result<tonic::Response<Self::BookSummaryStream>, tonic::Status> {
        let result_stream = stream::unfold(self.ticks.clone(), |mut ticks| async move {
            if ticks.changed().await.is_ok() {
                let cloned_summary = ticks.borrow().clone();
                Some((cloned_summary, ticks))
            } else {
                None
            }
        })
        .map(Result::<_, tonic::Status>::Ok);
        Ok(tonic::Response::new(Box::pin(result_stream)))
    }
}
