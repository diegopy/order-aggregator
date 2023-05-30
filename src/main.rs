use ::config::{Config, Environment};
use config::{File, FileFormat};

use configuration::AppConfig;

use futures::Future;
use log::info;
use tokio::sync::{mpsc, watch};
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

use crate::orderbook::Summary;
use crate::server::grpc_server;

mod aggregator;
mod binance;
mod bitstamp;
mod common;
mod configuration;
mod server;

pub mod orderbook {
    tonic::include_proto!("orderbook");
}

#[tokio::main]
async fn main() {
    dotenvy::dotenv().expect("read .env file");
    env_logger::init();
    let config_reader = Config::builder()
        .add_source(File::with_name("config").format(FileFormat::Toml))
        .add_source(Environment::with_prefix("APP"))
        .build()
        .expect("config builder");
    let app_config: AppConfig = config_reader.try_deserialize().expect("deserialize config");
    println!("config: {:?}", app_config);
    let (sender, receiver) = mpsc::channel(5);
    let (summary_sender, summary_receiver) = watch::channel(Summary::default());
    let stop_signal = CancellationToken::new();
    let mut tasks = vec![];
    spawn_task(
        &mut tasks,
        "bitstamp_stream",
        bitstamp::bitstamp_stream(
            app_config.bitstamp,
            app_config.symbol.clone(),
            sender.clone(),
        ),
    );
    spawn_task(
        &mut tasks,
        "binance_stream",
        binance::binance_stream(
            app_config.binance,
            app_config.symbol.clone(),
            sender.clone(),
        ),
    );
    spawn_task(
        &mut tasks,
        "orders_aggregator",
        aggregator::orders_aggregator(receiver, summary_sender, app_config.max_aggregated_levels),
    );
    spawn_task(
        &mut tasks,
        "grpc_server",
        grpc_server(summary_receiver, stop_signal),
    );
    for task in tasks {
        task.await
            .expect("task panicked")
            .expect("task finished with error");
    }
}

fn spawn_task<T: Future<Output = Result<(), anyhow::Error>>>(
    tasks: &mut Vec<JoinHandle<T::Output>>,
    task_name: &'static str,
    future: T,
) where
    T: Future + Send + 'static,
{
    let block = async move {
        let result = future.await;
        info!("task {} ended with {:?}", task_name, result);
        result
    };
    let task_handle = tokio::spawn(block);
    tasks.push(task_handle);
}
