use anyhow::Result;
use parking_lot::RwLock;

use std::sync::Arc;

mod binance_ws;
mod orderbook;
mod polymarket;

use orderbook::Orderbook;

pub struct SpawnExecutor;

impl<Fut> hyper::rt::Executor<Fut> for SpawnExecutor
where
    Fut: std::future::Future + Send + 'static,
    Fut::Output: Send + 'static,
{
    fn execute(&self, fut: Fut) {
        tokio::task::spawn(fut);
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    // Create empty orderbook
    let orderbook = Arc::new(RwLock::new(Orderbook::default()));

    // Spawn polymarket websocket as background task
    let polymarket_handle = tokio::spawn(async move {
        if let Err(e) = polymarket::websocket::connect(orderbook).await {
            eprintln!("Polymarket websocket error: {}", e);
        }
    });

    // Wait for all tasks to complete
    let _ = polymarket_handle.await;

    Ok(())
}
