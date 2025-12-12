use anyhow::Error;
use reqwest::{Client, StatusCode};
use std::sync::Arc;

use crate::polymarket::gamma::types::{ListMarketParams, Markets};

#[derive(Clone, Debug)]
pub struct PolymarketGammaClient {
    client: Arc<Client>,
    base_url: String,
}

impl PolymarketGammaClient {
    pub fn new(client: Arc<Client>, base_url: String) -> Self {
        Self { client, base_url }
    }

    pub async fn list_markets(
        &self,
        query_params: ListMarketParams,
    ) -> Result<Vec<Markets>, Error> {
        let url = format!("{}/markets", self.base_url);

        let response = self.client.get(&url).query(&query_params).send().await?;
        match response.status() {
            StatusCode::OK => {
                let markets: Vec<Markets> = response.json().await.map_err(|e| Error::msg(e.to_string()))?;
                Ok(markets)
            }
            _ => {
                return Err(Error::msg("Failed to list markets"));
            }
        }
    }
}
