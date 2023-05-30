use anyhow::{bail, Context};
use futures::{SinkExt, StreamExt};
use log::{info, warn};
use tokio::sync::mpsc::Sender;
use tokio_tungstenite::tungstenite::Message;

use crate::{aggregator::ExchangeOrders, common::OrderBookData, configuration::BinanceConfig};

pub(crate) async fn binance_stream(
    config: BinanceConfig,
    symbol: String,
    orders_sender: Sender<ExchangeOrders>,
) -> anyhow::Result<()> {
    let max_levels = config.depth as u8;
    let interval = config.interval as u16;
    let url = config
        .url
        .join("ws/")
        .context("joining url: ws")?
        .join(&format!("{symbol}@depth{max_levels}@{interval}ms"))
        .context("joining url: params")?;
    info!("Connecting to {}", url);
    let (binance_ws, _) = tokio_tungstenite::connect_async(url)
        .await
        .context("establishing connection")?;
    let (mut binance_writer, mut binance_reader) = binance_ws.split();
    while let Some(read_result) = binance_reader.next().await {
        match read_result.context("reading packet")? {
            Message::Text(message_text) => {
                let order_book: OrderBookData = serde_json::from_str(&message_text)
                    .with_context(|| format!("parsing message: {message_text}"))?;
                let send_result = orders_sender
                    .send(order_book.into_exchange_orders("binance".to_owned(), max_levels.into()))
                    .await;
                if send_result.is_err() {
                    info!("channel closed. Exiting.");
                    break;
                }
            }
            Message::Binary(_) => {
                bail!(format!("unsupported binary message"))
            }
            Message::Ping(payload) => {
                binance_writer
                    .send(Message::Pong(payload))
                    .await
                    .context("sending pong message")?;
            }
            Message::Pong(_) => warn!("got PONG. Ignoring."),
            Message::Close(_) => {
                info!("got CLOSE. Closing and restarting.");
                bail!("Closed, restarting");
            }
            Message::Frame(_) => warn!("got FRAME. Ignoring."),
        }
    }
    Ok(())
}
