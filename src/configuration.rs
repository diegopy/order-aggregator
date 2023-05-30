use std::time::Duration;

use serde::Deserialize;
use url::Url;

#[derive(Debug, Deserialize, Clone)]
pub(crate) struct AppConfig {
    pub symbol: String,
    pub binance: BinanceConfig,
    pub bitstamp: BitstampConfig,
    pub server: Server,
    pub backoff: BackoffConfig,
    pub max_aggregated_levels: usize,
    pub channel_size: usize,
}

#[derive(Debug, Deserialize, Clone)]
pub(crate) struct BinanceConfig {
    pub url: Url,
    pub depth: BinanceDepth,
    pub interval: BinanceInterval,
    #[serde(default)]
    pub sort: bool,
}

#[derive(Debug, Deserialize, Copy, Clone)]
pub enum BinanceDepth {
    D5 = 5,
    D10 = 10,
    D20 = 20,
}

#[derive(Debug, Deserialize, Copy, Clone)]
pub enum BinanceInterval {
    I100 = 100,
    I1000 = 1000,
}

#[derive(Debug, Deserialize, Clone)]
pub struct BitstampConfig {
    pub url: Url,
    pub depth: usize,
    #[serde(default)]
    pub sort: bool,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Server {
    pub port: u16,
}

#[derive(Debug, Deserialize, Clone)]
pub struct BackoffConfig {
    pub retries: u32,
    #[serde(with = "humantime_serde")]
    pub min: Duration,
    #[serde(with = "humantime_serde")]
    pub max: Duration,
}

#[cfg(test)]
mod tests {

    use config::{Config, File, FileFormat};

    use super::*;

    #[test]
    fn test_parse() {
        let config_string = r#"
symbol = "ethbtc"
max_aggregated_levels = 10
channel_size = 5

[binance]
url = "wss://stream.binance.com:9443"
depth = "D10"
interval = "I100"

[bitstamp]
url = "wss://ws.bitstamp.net"
depth = 10

[server]
port = 5000

[backoff]
retries = 5
min = "100ms"
max = "10s"
"#;
        let config_reader = Config::builder()
            .add_source(File::from_str(config_string, FileFormat::Toml))
            .build()
            .expect("building config");
        let _app_config: AppConfig = config_reader.try_deserialize().expect("deserialize config");
    }
}
