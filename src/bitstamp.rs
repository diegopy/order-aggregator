use anyhow::{bail, Context};
use futures::{SinkExt, StreamExt};
use log::{info, warn};
use serde::Deserialize;
use serde_json::json;
use tokio::sync::mpsc::Sender;
use tokio_tungstenite::tungstenite::Message;

use crate::{aggregator::ExchangeOrders, common::OrderBookData, configuration::BitstampConfig};

pub(crate) async fn bitstamp_stream(
    config: BitstampConfig,
    symbol: String,
    orders_sender: Sender<ExchangeOrders>,
) -> anyhow::Result<()> {
    let (bitstamp_ws, _) = tokio_tungstenite::connect_async(&config.url)
        .await
        .context("connecting to ws")?;
    let (mut bitstamp_writer, mut bitstamp_reader) = bitstamp_ws.split();
    let channel_name = format!("order_book_{symbol}");
    let subscribe_message = json!({
        "event": "bts:subscribe",
        "data": {
            "channel": channel_name,
        }
    });
    bitstamp_writer
        .send(
            serde_json::to_string(&subscribe_message)
                .expect("can't fail serialize")
                .into(),
        )
        .await
        .context("subscribing to bitstamp stream")?;
    while let Some(read_result) = bitstamp_reader.next().await {
        match read_result.context("reading packet")? {
            Message::Text(message_text) => {
                let bitstamp_message: BitstampMessage = serde_json::from_str(&message_text)
                    .with_context(|| format!("parsing message: {message_text}"))?;
                if let BitstampMessage::Data { data: order_book } = bitstamp_message {
                    let send_result = orders_sender
                        .send(order_book.into_exchange_orders("bitstamp".to_owned(), config.depth))
                        .await;
                    if send_result.is_err() {
                        info!("binance stream: channel closed. Exiting.");
                        break;
                    }
                } else {
                    info!("ignoring non-data message")
                }
            }
            Message::Binary(_) => {
                bail!(format!("binance stream: unsupported binary message"))
            }
            Message::Ping(payload) => {
                bitstamp_writer
                    .send(Message::Pong(payload))
                    .await
                    .context("binance stream: sending pong message")?;
            }
            Message::Pong(_) => warn!("binance stream: got PONG. Ignoring."),
            Message::Close(_) => {
                info!("binance stream: got CLOSE. Closing and restarting.");
                bail!("binance stream: Closed, restarting");
            }
            Message::Frame(_) => warn!("binance stream: got FRAME. Ignoring."),
        }
    }
    Ok(())
}

#[derive(Deserialize)]
#[serde(tag = "event", rename_all = "lowercase")]
enum BitstampMessage {
    Data {
        data: OrderBookData,
    },
    #[serde(other)]
    Unknown,
}
