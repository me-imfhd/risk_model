use chrono::{Timelike, Utc};

use crate::risk_model::RiskCalculationError;

use super::yield_data::{Metrics, MetricsResponse};

pub async fn get_total_borrows_and_supply() -> Result<(f64, f64), RiskCalculationError> {
    let nearest_hour = Utc::now()
        .with_minute(0)
        .unwrap()
        .with_second(0)
        .unwrap()
        .with_nanosecond(0)
        .unwrap();
    let start = nearest_hour - chrono::Duration::hours(24);
    let url = format!(
        "https://api.kamino.finance/kamino-market/H6rHXmXoCQvq8Ue81MqNh7ow5ysPa1dSozwW3PU1dDH6/reserves/6gTJfuPHEg6uRAijRkMqNc9kan4sVZejKMxmvx2grT1p/metrics/history?env=mainnet-beta&start={}Z&end={}Z&frequency=hour",
        start.format("%Y-%m-%d"),
        nearest_hour.format("%Y-%m-%d")
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

    // Get the latest utilization rat
    let Metrics {
        ref total_borrows,
        ref total_supply,
        ..
    } = metrics_data
        .history
        .iter()
        .last()
        .ok_or(RiskCalculationError::CustomError(
            "No history data available".to_string(),
        ))?
        .metrics;
    let total_borrows = total_borrows
        .parse::<f64>()
        .map_err(|e| RiskCalculationError::ParseError(e.to_string()))?;
    let total_supply = total_supply
        .parse::<f64>()
        .map_err(|e| RiskCalculationError::ParseError(e.to_string()))?;
    Ok((total_borrows, total_supply))
}
