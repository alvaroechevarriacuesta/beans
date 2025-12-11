use serde::Deserialize;

type PriceLevel = u64;
type Quantity = u64;

const PRICE_DECIMALS: u32 = 2;
const SIZE_DECIMALS: u32 = 1;

#[derive(Debug, Deserialize)]
struct IncomingPriceChangeMessage {
    market: String,
    price_changes: Vec<PriceChange>,
    #[serde(deserialize_with = "deserialize_timestamp")]
    timestamp: u64,
}

#[derive(Debug, Deserialize)]
struct PriceChange {
    asset_id: String,
    #[serde(deserialize_with = "deserialize_price")]
    price: PriceLevel,
    #[serde(deserialize_with = "deserialize_size")]
    size: Quantity,
}

pub struct UpdateBook {
    pub market: String,
    pub asset_id: String,
    pub price: PriceLevel,
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
