#![allow(unused)]
use chrono::{DateTime, Timelike, Utc};
use serde::Deserialize;

use crate::risk_model::RiskCalculationError;

#[derive(Debug, Deserialize)]
pub struct MetricsResponse {
    pub reserve: String,
    pub history: Vec<HistoryEntry>,
}

#[derive(Debug, Deserialize)]
pub struct HistoryEntry {
    pub timestamp: String,
    pub metrics: Metrics,
}

#[derive(Debug, Deserialize)]
pub struct Metrics {
    #[serde(rename = "borrowInterestAPY")]
    pub borrow_interest_apy: f64,
    #[serde(rename = "supplyInterestAPY")]
    pub supply_interest_apy: f64,
    #[serde(rename = "totalBorrows")]
    pub total_borrows: String,
    #[serde(rename = "totalSupply")]
    pub total_supply: String,
}

#[derive(Debug)]
pub struct YieldData {
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
    pub yields_percent: Vec<f64>,
    pub utilization_rates_percent: Vec<f64>,
}

pub async fn fetch_yield_and_utilization_rates() -> Result<YieldData, RiskCalculationError> {
    let end = Utc::now()
        .with_minute(0)
        .unwrap()
        .with_second(0)
        .unwrap()
        .with_nanosecond(0)
        .unwrap();
    let start = end - chrono::Duration::hours(24);
    let url = format!(
        "https://api.kamino.finance/kamino-market/H6rHXmXoCQvq8Ue81MqNh7ow5ysPa1dSozwW3PU1dDH6/reserves/6gTJfuPHEg6uRAijRkMqNc9kan4sVZejKMxmvx2grT1p/metrics/history?env=mainnet-beta&start={}Z&end={}Z&frequency=hour",
        start.format("%Y-%m-%d"),
        end.format("%Y-%m-%d")
    );

    let response = reqwest::get(&url)
        .await
        .map_err(|e| RiskCalculationError::RequestError(e))?;
    let raw_data = response
        .text()
        .await
        .map_err(|e| RiskCalculationError::RequestError(e))?;
    let metrics_data: MetricsResponse =
        serde_json::from_str(&raw_data).map_err(|e| RiskCalculationError::SerdeError(e))?;

    let mut yields: Vec<f64> = Vec::new();
    let mut utilization_rates: Vec<f64> = Vec::new();

    for entry in metrics_data.history {
        yields.push(entry.metrics.supply_interest_apy * 100.0); // Convert to percentage

        // Calculate utilization rate
        let total_borrows = entry
            .metrics
            .total_borrows
            .parse::<f64>()
            .map_err(|e| RiskCalculationError::ParseError(e.to_string()))?;
        let total_supply = entry
            .metrics
            .total_supply
            .parse::<f64>()
            .map_err(|e| RiskCalculationError::ParseError(e.to_string()))?;
        let utilization_rate = if total_supply > 0.0 {
            (total_borrows / total_supply) * 100.0 // Convert to percentage
        } else {
            0.0
        };
        utilization_rates.push(utilization_rate);
    }

    if yields.is_empty() {
        return Err(RiskCalculationError::CustomError(
            "No yield data available".to_string(),
        ));
    }

    Ok(YieldData {
        start,
        end,
        yields_percent: yields,
        utilization_rates_percent: utilization_rates,
    })
}
