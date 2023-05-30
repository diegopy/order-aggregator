use std::str::FromStr;

use ordered_float::NotNan;
use serde::Deserialize;

use crate::aggregator;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct OrderBookData {
    pub bids: Vec<Level>,
    pub asks: Vec<Level>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", try_from = "(String, String)")]
pub(crate) struct Level {
    pub price: NotNan<f64>,
    pub quantity: NotNan<f64>,
}

impl TryFrom<(String, String)> for Level {
    type Error = <NotNan<f64> as FromStr>::Err;

    fn try_from(value: (String, String)) -> Result<Self, Self::Error> {
        Ok(Self {
            price: value.0.parse()?,
            quantity: value.1.parse()?,
        })
    }
}

impl OrderBookData {
    pub fn into_exchange_orders(
        self,
        exchange_name: String,
        max_levels: usize,
    ) -> aggregator::ExchangeOrders {
        aggregator::ExchangeOrders {
            asks: self
                .asks
                .into_iter()
                .take(max_levels)
                .map(|l| l.into_aggregator_level(exchange_name.clone()))
                .collect(),
            bids: self
                .bids
                .into_iter()
                .take(max_levels)
                .map(|l| l.into_aggregator_level(exchange_name.clone()))
                .collect(),
            exchange_name,
        }
    }
}

impl Level {
    pub fn into_aggregator_level(self, exchange_name: String) -> aggregator::Level {
        aggregator::Level {
            exchange_name,
            price: self.price,
            amount: self.quantity,
        }
    }
}
