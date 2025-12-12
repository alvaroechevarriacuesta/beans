use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct ListMarketParams {
    pub market_id: Option<String>,
    pub market_name: Option<String>,
    pub market_type: Option<String>,
    pub market_status: Option<String>,
    pub market_created_at: Option<String>,
    pub market_updated_at: Option<String>,
    pub market_deleted_at: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Markets {
    pub id: String,
    pub name: String,
    pub description: String,
    pub image_url: String,
    pub created_at: String,
    pub updated_at: String,
    pub deleted_at: String,
}
