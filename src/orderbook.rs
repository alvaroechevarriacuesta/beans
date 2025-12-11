use parking_lot::RwLock;
use serde::Deserialize;
use std::cmp::Reverse;
use std::collections::BTreeMap;

type PriceLevel = u64;
type Quantity = u64;

const PRICE_DECIMALS: u32 = 2;
const SIZE_DECIMALS: u32 = 1;

#[derive(Debug, Deserialize)]
struct LevelEntry {
    #[serde(deserialize_with = "deserialize_price")]
    price: PriceLevel,
    #[serde(deserialize_with = "deserialize_size")]
    size: Quantity,
}

#[derive(Debug, Deserialize)]
struct IncomingOrderBookMessage {
    market: String,
    asset_id: String,
    #[serde(deserialize_with = "deserialize_timestamp")]
    timestamp: u64,
    bids: Vec<LevelEntry>,
    asks: Vec<LevelEntry>,
    #[allow(dead_code)]
    hash: String,
}

#[derive(Debug, Default)]
pub struct Orderbook {
    pub market: String,
    pub asset_id: String,
    pub timestamp: u64,
    pub asks: BTreeMap<PriceLevel, Quantity>,
    pub bids: BTreeMap<Reverse<PriceLevel>, Quantity>,
}

impl Orderbook {
    pub fn apply_snapshot(&mut self, snapshot: Orderbook) {
        self.market = snapshot.market;
        self.asset_id = snapshot.asset_id;
        self.timestamp = snapshot.timestamp;
        self.asks = snapshot.asks;
        self.bids = snapshot.bids;
    }
    pub fn update_from_bytes(&mut self, bytes: &[u8]) -> Result<(), serde_json::Error> {
        let snapshot = Orderbook::from_bytes(bytes)?;
        self.apply_snapshot(snapshot);
        Ok(())
    }

    

    // Helper function
    pub fn from_bytes(bytes: &[u8]) -> Result<Orderbook, serde_json::Error> {
        let msg: IncomingOrderBookMessage = serde_json::from_slice(bytes)?;

        let asks: BTreeMap<PriceLevel, Quantity> = msg
            .asks
            .into_iter()
            .filter(|e| e.size > 0)
            .map(|e| (e.price, e.size))
            .collect();

        let bids: BTreeMap<Reverse<PriceLevel>, Quantity> = msg
            .bids
            .into_iter()
            .filter(|e| e.size > 0)
            .map(|e| (Reverse(e.price), e.size))
            .collect();

        Ok(Orderbook {
            market: msg.market,
            asset_id: msg.asset_id,
            timestamp: msg.timestamp,
            asks,
            bids,
        })
    }

    /// Get the best bid (highest price willing to buy)
    #[inline]
    pub fn best_bid(&self) -> Option<(PriceLevel, Quantity)> {
        self.bids.first_key_value().map(|(k, &v)| (k.0, v))
    }

    /// Get the best ask (lowest price willing to sell)
    #[inline]
    pub fn best_ask(&self) -> Option<(PriceLevel, Quantity)> {
        self.asks.first_key_value().map(|(&k, &v)| (k, v))
    }

    /// Get the mid price (average of best bid and ask)
    #[inline]
    pub fn mid_price(&self) -> Option<PriceLevel> {
        match (self.best_bid(), self.best_ask()) {
            (Some((bid, _)), Some((ask, _))) => Some((bid + ask) / 2),
            _ => None,
        }
    }

    /// Get the spread between best ask and best bid
    #[inline]
    pub fn spread(&self) -> Option<PriceLevel> {
        match (self.best_bid(), self.best_ask()) {
            (Some((bid, _)), Some((ask, _))) => Some(ask.saturating_sub(bid)),
            _ => None,
        }
    }
}

/// Deserialize string price like "0.33" to integer 33 (in cents)
fn deserialize_price<'de, D>(deserializer: D) -> Result<PriceLevel, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let s: &str = Deserialize::deserialize(deserializer)?;
    parse_decimal_to_int(s, PRICE_DECIMALS).map_err(serde::de::Error::custom)
}

/// Deserialize string size like "4142.5" to integer 41425 (in tenths)
fn deserialize_size<'de, D>(deserializer: D) -> Result<Quantity, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let s: &str = Deserialize::deserialize(deserializer)?;
    parse_decimal_to_int(s, SIZE_DECIMALS).map_err(serde::de::Error::custom)
}

/// Deserialize string timestamp to u64
fn deserialize_timestamp<'de, D>(deserializer: D) -> Result<u64, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let s: &str = Deserialize::deserialize(deserializer)?;
    s.parse::<u64>().map_err(serde::de::Error::custom)
}

/// Parse a decimal string to integer with fixed precision
/// e.g., "0.33" with 2 decimals -> 33
/// e.g., "4142.5" with 1 decimal -> 41425
fn parse_decimal_to_int(s: &str, decimals: u32) -> Result<u64, &'static str> {
    let multiplier = 10u64.pow(decimals);

    if let Some(dot_pos) = s.find('.') {
        let int_part: u64 = s[..dot_pos].parse().map_err(|_| "invalid integer part")?;
        let frac_str = &s[dot_pos + 1..];
        let frac_len = frac_str.len() as u32;

        let frac_part: u64 = frac_str.parse().map_err(|_| "invalid fractional part")?;

        // Scale the fractional part to match our precision
        let scaled_frac = if frac_len < decimals {
            frac_part * 10u64.pow(decimals - frac_len)
        } else if frac_len > decimals {
            frac_part / 10u64.pow(frac_len - decimals)
        } else {
            frac_part
        };

        Ok(int_part * multiplier + scaled_frac)
    } else {
        // No decimal point - just an integer
        let int_part: u64 = s.parse().map_err(|_| "invalid integer")?;
        Ok(int_part * multiplier)
    }
}
