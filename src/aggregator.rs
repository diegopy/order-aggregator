use std::{
    cmp::Reverse,
    collections::{BinaryHeap, HashMap},
};

use anyhow::Context;
use log::{debug, info};
use ordered_float::NotNan;
use tokio::sync::{mpsc::Receiver, watch::Sender};

use crate::orderbook::{self, Summary};

#[derive(Debug)]
pub(crate) struct ExchangeOrders {
    pub exchange_name: String,
    pub asks: Vec<Level>,
    pub bids: Vec<Level>,
}

#[derive(Debug, Ord, PartialOrd, PartialEq, Eq)]
pub(crate) struct Level {
    pub price: NotNan<f64>,
    pub amount: NotNan<f64>,
    pub exchange_name: String,
}

pub(crate) async fn orders_aggregator(
    mut receiver: Receiver<ExchangeOrders>,
    sender: Sender<Summary>,
    max_levels: usize,
) -> anyhow::Result<()> {
    let mut exchanges = HashMap::new();
    while let Some(exchange_orders) = receiver.recv().await {
        debug!(
            "received orders: exchange:{} len: {}",
            exchange_orders.exchange_name,
            exchange_orders.asks.len()
        );
        exchanges.insert(exchange_orders.exchange_name.clone(), exchange_orders);
        let summary = sort_orders_and_calculate_spread(&exchanges, max_levels);
        if sender.send(summary).context("sending summary").is_err() {
            info!("sender channel closed. Exiting");
            break;
        }
    }
    Ok(())
}

fn sort_orders_and_calculate_spread(
    exchanges: &HashMap<String, ExchangeOrders>,
    max_levels: usize,
) -> Summary {
    let (all_bids, all_asks): (Vec<_>, Vec<_>) = exchanges
        .values()
        .map(|exchange| (exchange.bids.as_slice(), exchange.asks.as_slice()))
        .unzip();
    let sorted_bids = sort_merged(&all_bids, max_levels, |x| x);
    let sorted_asks = sort_merged(&all_asks, max_levels, Reverse);
    let sorted_bids: Vec<orderbook::Level> = sorted_bids.into_iter().map(Into::into).collect();
    let sorted_asks: Vec<orderbook::Level> = sorted_asks
        .into_iter()
        .map(|rev_ask| rev_ask.0.into())
        .collect();
    let spread = sorted_asks.first().map_or(0.0, |ask| ask.price)
        - sorted_bids.first().map_or(0.0, |ask| ask.price);
    Summary {
        bids: sorted_bids,
        asks: sorted_asks,
        spread,
    }
}

fn sort_merged<'a, T, F, U>(levels: &[&'a [T]], max_levels: usize, f: F) -> Vec<U>
where
    T: Ord + 'a,
    F: Fn(&'a T) -> U,
    U: Ord + 'a,
{
    let mut heap = BinaryHeap::new();
    let mut iterators: Vec<_> = levels.iter().map(|x| x.iter()).collect();
    for (i, iter) in iterators.iter_mut().enumerate() {
        if let Some(item) = iter.next() {
            heap.push((f(item), i))
        }
    }
    let mut result = Vec::with_capacity(max_levels);
    while let Some((item, i)) = heap.pop() {
        result.push(item);
        if let Some(item) = iterators[i].next() {
            heap.push((f(item), i));
        }
    }
    result
}

impl From<&Level> for orderbook::Level {
    fn from(value: &Level) -> Self {
        Self {
            price: value.price.into(),
            amount: value.amount.into(),
            exchange: value.exchange_name.clone(),
        }
    }
}
