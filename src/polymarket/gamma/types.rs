use serde::{Deserialize, Deserializer, Serialize};

#[derive(Debug, Default)]
pub struct ListMarketParams {
    pub tag_id: Option<u64>,
    pub order: Option<String>,
    pub ascending: Option<bool>,
}

impl ListMarketParams {
    pub fn to_query_params(&self) -> Vec<(&'static str, String)> {
        let mut params = Vec::new();
        if let Some(tag_id) = self.tag_id {
            params.push(("tag_id", tag_id.to_string()));
        }
        if let Some(ref order) = self.order {
            params.push(("order", order.clone()));
        }
        if let Some(ascending) = self.ascending {
            params.push(("ascending", ascending.to_string()));
        }
        params
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Token {
    pub token_id: String,
    pub outcome: String,
}

#[derive(Debug, Deserialize)]
struct RawMarket {
    pub id: String,
    #[serde(alias = "conditionId")]
    pub condition_id: String,
    pub slug: String,
    #[serde(alias = "outcomes", deserialize_with = "deserialize_stringified_array")]
    pub outcomes: Vec<String>,
    #[serde(
        alias = "clobTokenIds",
        deserialize_with = "deserialize_stringified_array"
    )]
    pub clob_token_ids: Vec<String>,
    pub active: bool,
    pub closed: bool,
    #[serde(alias = "endDate")]
    pub end_date: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Market {
    pub id: String,
    pub condition_id: String,
    pub slug: String,
    pub tokens: Vec<Token>,
    pub active: bool,
    pub closed: bool,
    pub end_date: Option<String>,
}

impl<'de> Deserialize<'de> for Market {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = RawMarket::deserialize(deserializer)?;

        // Combine outcomes and clob_token_ids positionally into Token objects
        let tokens: Vec<Token> = raw
            .clob_token_ids
            .into_iter()
            .zip(raw.outcomes.into_iter())
            .map(|(token_id, outcome)| Token { token_id, outcome })
            .collect();

        Ok(Market {
            id: raw.id,
            condition_id: raw.condition_id,
            slug: raw.slug,
            tokens,
            active: raw.active,
            closed: raw.closed,
            end_date: raw.end_date,
        })
    }
}

/// Deserializes a JSON-encoded string array like "[\"Yes\", \"No\"]" into Vec<String>
fn deserialize_stringified_array<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
    D: Deserializer<'de>,
{
    let s: String = Deserialize::deserialize(deserializer)?;
    serde_json::from_str(&s).map_err(serde::de::Error::custom)
}
