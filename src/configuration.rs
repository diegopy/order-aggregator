use serde::Deserialize;
use url::Url;

#[derive(Debug, Deserialize)]
pub(crate) struct AppConfig {
    pub symbol: String,
    pub binance: BinanceConfig,
    pub bitstamp: BitstampConfig,
    pub max_aggregated_levels: usize,
}

#[derive(Debug, Deserialize)]
pub(crate) struct BinanceConfig {
    pub url: Url,
    pub depth: BinanceDepth,
    pub interval: BinanceInterval,
}

#[derive(Debug, Deserialize)]
pub enum BinanceDepth {
    D5 = 5,
    D10 = 10,
    D20 = 20,
}

#[derive(Debug, Deserialize)]
pub enum BinanceInterval {
    I100 = 100,
    I1000 = 1000,
}

#[derive(Debug, Deserialize)]
pub struct BitstampConfig {
    pub url: Url,
    pub depth: usize,
}

#[cfg(test)]
mod tests {

    use config::{Config, File, FileFormat};

    use super::*;

    #[test]
    fn test_parse() {
        let config_string = r#"
symbol = "ethbtc"

[binance]
url = "wss://stream.binance.com:9443"
depth = "D10"
interval = "I100"

[bitstamp]
url = "wss://ws.bitstamp.net"
"#;
        let config_reader = Config::builder()
            .add_source(File::from_str(config_string, FileFormat::Toml))
            .build()
            .expect("building config");
        let _app_config: AppConfig = config_reader.try_deserialize().expect("deserialize config");
    }
}
