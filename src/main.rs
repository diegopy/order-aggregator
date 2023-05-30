use ::config::{Config, Environment};
use anyhow::bail;
use config::{File, FileFormat};

use configuration::{AppConfig, BackoffConfig};

use exponential_backoff::Backoff;
use futures::Future;
use log::{error, info};
use tokio::sync::{mpsc, watch};
use tokio::task::JoinHandle;
use tokio::time;
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
        .add_source(Environment::with_prefix("APP").separator("_"))
        .build()
        .expect("config builder");
    let app_config: AppConfig = config_reader.try_deserialize().expect("deserialize config");
    println!("config: {:?}", app_config);
    let (sender, receiver) = mpsc::channel(app_config.channel_size);
    let (summary_sender, summary_receiver) = watch::channel(Summary::default());
    let stop_signal = CancellationToken::new();
    let mut tasks = vec![];
    let cancellation_token = CancellationToken::new();
    let symbol = app_config.symbol.clone();
    let bitstamp_sender = sender.clone();
    spawn_task_backoff(
        &mut tasks,
        "bitstamp_stream",
        cancellation_token.clone(),
        &app_config.backoff,
        move || {
            bitstamp::bitstamp_stream(
                app_config.bitstamp.clone(),
                symbol.clone(),
                bitstamp_sender.clone(),
            )
        },
    );
    let symbol = app_config.symbol.clone();
    spawn_task_backoff(
        &mut tasks,
        "binance_stream",
        cancellation_token.clone(),
        &app_config.backoff,
        move || binance::binance_stream(app_config.binance.clone(), symbol.clone(), sender.clone()),
    );
    spawn_task(
        &mut tasks,
        "orders_aggregator",
        cancellation_token.clone(),
        move || {
            aggregator::orders_aggregator(
                receiver,
                summary_sender,
                app_config.max_aggregated_levels,
            )
        },
    );
    spawn_task(
        &mut tasks,
        "grpc_server",
        cancellation_token.clone(),
        || grpc_server(summary_receiver, stop_signal, app_config.server),
    );
    for task in tasks {
        match task.await {
            Ok(task_result) => {
                if let Err(e) = task_result {
                    error!("Task finished with error: {}", e)
                }
            }
            Err(e) => error!("Error joining task: {}", e),
        }
    }
    info!("All tasks joined. Exiting process")
}

fn spawn_task_backoff<F, T: Future<Output = Result<(), anyhow::Error>> + Send + 'static>(
    tasks: &mut Vec<JoinHandle<T::Output>>,
    task_name: &'static str,
    cancellation_token: CancellationToken,
    backoff_config: &BackoffConfig,
    f: F,
) where
    F: Fn() -> T + Send + 'static,
{
    let backoff = Backoff::new(
        backoff_config.retries,
        backoff_config.min,
        Some(backoff_config.max),
    );
    let async_block = async move {
        for duration in &backoff {
            tokio::select! {
                result = f() => {
                    match result {
                        Ok(()) => {
                            info!("task {} ended gracefully. Shutting down other tasks.", task_name);
                        },
                        Err(ref e) => {
                            error!("task {} ended with error {}. Backing off.", task_name, e);
                            time::sleep(duration).await;
                        }
                    }
                }
                _ = cancellation_token.cancelled() => {
                    info!("task {} cancelled. Exiting", task_name);
                    bail!("task {} cancelled", task_name)
                }
            }
        }
        cancellation_token.cancel();
        bail!("backoff retries exhausted for task {}", task_name);
    };
    let task_handle = tokio::spawn(async_block);
    tasks.push(task_handle);
}

fn spawn_task<F, T: Future<Output = Result<(), anyhow::Error>> + Send + 'static>(
    tasks: &mut Vec<JoinHandle<T::Output>>,
    task_name: &'static str,
    cancellation_token: CancellationToken,
    f: F,
) where
    F: FnOnce() -> T + Send + 'static,
{
    let async_block = async move {
        tokio::select! {
            result = f() => {
                match result {
                    Ok(()) => {
                        info!("task {} ended gracefully. Shutting down other tasks.", task_name);
                    },
                    Err(ref e) => {
                        error!("task {} ended with error {}. Shutting down other tasks.", task_name, e)
                    }
                }
                cancellation_token.cancel();
                result
            }
            _ = cancellation_token.cancelled() => {
                info!("task {} cancelled. Exiting", task_name);
                bail!("task {} cancelled", task_name)
            }
        }
    };
    let task_handle = tokio::spawn(async_block);
    tasks.push(task_handle);
}
