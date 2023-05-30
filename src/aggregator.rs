use std::{
    cmp::Reverse,
    collections::{BinaryHeap, HashMap},
    convert,
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
    let sorted_bids = sort_merged(&all_bids, max_levels, convert::identity);
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
        if result.len() == max_levels {
            break;
        }
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

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn test_sorting_asks() {
        let a = [1, 5, 8];
        let b = [0, 2, 3, 9];
        let c = [4, 7];
        let values = [a.as_slice(), &b, &c];
        let res = sort_merged(&values, 3, Reverse);
        let res: Vec<_> = res.into_iter().map(|i| *i.0).collect();
        assert_eq!(vec![0, 1, 2], res);
    }

    #[test]
    fn test_sorting_bids() {
        let a = [level(10.0, "a"), level(9.0, "a"), level(8.9, "a")];
        let b = [level(12.0, "b"), level(9.0, "b"), level(8.9, "b")];
        let c = [level(11.0, "c"), level(9.0, "c"), level(8.9, "c")];
        let values = [a.as_slice(), &b, &c];
        let res = sort_merged(&values, 3, convert::identity);
        let res: Vec<_> = res
            .into_iter()
            .map(|i| Level {
                price: i.price,
                amount: i.amount,
                exchange_name: i.exchange_name.clone(),
            })
            .collect();
        assert_eq!(
            vec![level(12.0, "b"), level(11.0, "c"), level(10.0, "a")],
            res
        );
    }

    fn level(price: f64, exchange_name: &str) -> Level {
        Level {
            price: price.try_into().unwrap(),
            amount: NotNan::try_from(0.0).unwrap(),
            exchange_name: exchange_name.into(),
        }
    }

    #[test]
    fn test_sorting_and_spread() {
        let bid_a = vec![level(10.0, "a"), level(9.1, "a"), level(8.9, "a")];
        let bid_b = vec![level(12.0, "b"), level(9.2, "b"), level(8.9, "b")];
        let bid_c = vec![level(11.0, "c"), level(9.3, "c"), level(8.9, "c")];
        let ask_a = vec![level(15.0, "a"), level(20.9, "a"), level(80.9, "a")];
        let ask_b = vec![level(20.0, "b"), level(30.9, "b"), level(80.9, "b")];
        let ask_c = vec![level(13.5, "c"), level(18.8, "c"), level(80.9, "c")];
        let exc_a = ExchangeOrders {
            exchange_name: "a".to_owned(),
            asks: ask_a,
            bids: bid_a,
        };
        let exc_b = ExchangeOrders {
            exchange_name: "b".to_owned(),
            asks: ask_b,
            bids: bid_b,
        };
        let exc_c = ExchangeOrders {
            exchange_name: "c".to_owned(),
            asks: ask_c,
            bids: bid_c,
        };
        let mut exchanges = HashMap::new();
        exchanges.insert("a".to_owned(), exc_a);
        exchanges.insert("b".to_owned(), exc_b);
        exchanges.insert("c".to_owned(), exc_c);
        let summary = sort_orders_and_calculate_spread(&exchanges, 5);
        let expected = Summary {
            spread: 1.5,
            bids: vec![
                orderbook::Level {
                    exchange: "b".to_owned(),
                    price: 12.0,
                    amount: 0.0,
                },
                orderbook::Level {
                    exchange: "c".to_owned(),
                    price: 11.0,
                    amount: 0.0,
                },
                orderbook::Level {
                    exchange: "a".to_owned(),
                    price: 10.0,
                    amount: 0.0,
                },
                orderbook::Level {
                    exchange: "c".to_owned(),
                    price: 9.3,
                    amount: 0.0,
                },
                orderbook::Level {
                    exchange: "b".to_owned(),
                    price: 9.2,
                    amount: 0.0,
                },
            ],
            asks: vec![
                orderbook::Level {
                    exchange: "c".to_owned(),
                    price: 13.5,
                    amount: 0.0,
                },
                orderbook::Level {
                    exchange: "a".to_owned(),
                    price: 15.0,
                    amount: 0.0,
                },
                orderbook::Level {
                    exchange: "c".to_owned(),
                    price: 18.8,
                    amount: 0.0,
                },
                orderbook::Level {
                    exchange: "b".to_owned(),
                    price: 20.0,
                    amount: 0.0,
                },
                orderbook::Level {
                    exchange: "a".to_owned(),
                    price: 20.9,
                    amount: 0.0,
                },
            ],
        };
        assert_eq!(expected, summary);
    }
}
