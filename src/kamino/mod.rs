use axum::response::{IntoResponse, Response};
use deposit_conc::calculate_deposit_concentration;
use utilization_rate::get_utilization_rate;
use yield_data::fetch_yield_and_utilization_rates;

use crate::{
    liquidity_risk::calculate_liquidity_risk,
    risk_model::{get_seconds_until_next_hour, ProtocolRisk, RiskCalculationError},
    volatility_risk::calculate_lending_pool_risk,
};

mod deposit_conc;
mod utilization_rate;
mod yield_data;
pub struct KaminoRisk {
    redis_client: redis::Client,
}
use redis::AsyncCommands;

pub async fn kamino_risk() -> Response {
    let result = async {
        let kamino_risk = KaminoRisk {
            redis_client: redis::Client::open("redis://localhost:6379")
                .map_err(|e| RiskCalculationError::RedisError(e))?,
        };

        let liquidity_risk = kamino_risk.calculate_liquidity_risk(0.6, 0.4).await?;
        let volatility_risk = kamino_risk.calculate_volatility_risk(0.7, 0.3).await?;
        let protocol_risk = kamino_risk.calculate_protocol_risk().await?;

        let response = serde_json::json!({
            "liquidity_risk": liquidity_risk,
            "volatility_risk": volatility_risk,
            "protocol_risk": protocol_risk,
        });

        Ok::<_, RiskCalculationError>(axum::Json(response))
    }
    .await;

    match result {
        Ok(json) => json.into_response(),
        Err(e) => {
            let error_response = serde_json::json!({
                "error": e.to_string(),
                "error_type": format!("{:?}", e)
            });
            (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                axum::Json(error_response),
            )
                .into_response()
        }
    }
}
impl ProtocolRisk for KaminoRisk {
    async fn calculate_liquidity_risk(
        &self,
        weight_deposit_concentration_coefficient: f64,
        weight_utilization_coefficient: f64,
    ) -> Result<f64, RiskCalculationError> {
        let mut connection = self
            .redis_client
            .get_multiplexed_async_connection()
            .await
            .map_err(|e| RiskCalculationError::RedisError(e))?;

        // Try to get cached result first
        let cache_key = format!(
            "liquidity_risk:{}:{}",
            weight_deposit_concentration_coefficient, weight_utilization_coefficient
        );

        if let Ok(cached_result) = connection.get::<_, String>(&cache_key).await {
            return Ok(cached_result.parse::<f64>().unwrap());
        }

        let deposit_concentration = calculate_deposit_concentration().await.unwrap();
        let utilization_rate = get_utilization_rate().await?;
        let liquidity_risk = calculate_liquidity_risk(
            deposit_concentration,
            utilization_rate,
            weight_utilization_coefficient,
            weight_deposit_concentration_coefficient,
        );

        // Cache the result for 1 hour
        let _: () = connection
            .set_ex(
                &cache_key,
                liquidity_risk.to_string(),
                get_seconds_until_next_hour(),
            )
            .await
            .map_err(|e| RiskCalculationError::RedisError(e))?;

        Ok(liquidity_risk)
    }

    async fn calculate_volatility_risk(
        &self,
        weight_apy_coefficient: f64,
        weight_utilization_coefficient: f64,
    ) -> Result<f64, RiskCalculationError> {
        let mut connection = self
            .redis_client
            .get_multiplexed_async_connection()
            .await
            .map_err(|e| RiskCalculationError::RedisError(e))?;

        // Try to get cached result first
        let cache_key = format!(
            "volatility_risk:{}:{}",
            weight_apy_coefficient, weight_utilization_coefficient
        );

        if let Ok(cached_result) = connection.get::<_, String>(&cache_key).await {
            return Ok(cached_result.parse::<f64>().unwrap());
        }

        let data = fetch_yield_and_utilization_rates().await.unwrap();
        let volatility_risk = calculate_lending_pool_risk(
            data.yields_percent,
            data.utilization_rates_percent,
            weight_apy_coefficient,
            weight_utilization_coefficient,
        )
        .ok_or(RiskCalculationError::CustomError(
            "Insufficient data".to_string(),
        ))?;

        // Cache the result for 1 hour
        let _: () = connection
            .set_ex(
                &cache_key,
                volatility_risk.to_string(),
                get_seconds_until_next_hour(),
            )
            .await
            .map_err(|e| RiskCalculationError::RedisError(e))?;

        Ok(volatility_risk)
    }

    async fn calculate_protocol_risk(&self) -> Result<f64, RiskCalculationError> {
        let mut connection = self
            .redis_client
            .get_multiplexed_async_connection()
            .await
            .map_err(|e| RiskCalculationError::RedisError(e))?;

        let cache_key = "protocol_risk";

        if let Ok(cached_result) = connection.get::<_, String>(cache_key).await {
            return Ok(cached_result.parse::<f64>().unwrap());
        }

        // Constant protocol risk for Kamino
        let protocol_risk = 0.1; // 10% base protocol risk

        // Cache the result for 1 hour
        let _: () = connection
            .set_ex(
                cache_key,
                protocol_risk.to_string(),
                get_seconds_until_next_hour(),
            )
            .await
            .map_err(|e| RiskCalculationError::RedisError(e))?;

        Ok(protocol_risk)
    }
}

#[cfg(test)]
mod kamino_tests {
    use super::{
        deposit_conc::calculate_deposit_concentration, utilization_rate::get_utilization_rate,
        yield_data::fetch_yield_and_utilization_rates,
    };
    use crate::{
        liquidity_risk::calculate_liquidity_risk, volatility_risk::calculate_lending_pool_risk,
    };
    #[tokio::test]
    async fn test_liquidity_risk() {
        let utilization_weight = 0.6;
        let deposit_concentration_weight = 0.4;
        // Get deposit concentration
        let deposit_concentration = calculate_deposit_concentration().await.unwrap();
        println!("Deposit Concentration: {:?}", deposit_concentration);
        // Get utilization rate
        let utilization_rate = get_utilization_rate().await.unwrap();
        println!("Utilization Rate: {:?}", utilization_rate);

        let liquidity_risk = calculate_liquidity_risk(
            deposit_concentration,
            utilization_rate,
            utilization_weight,
            deposit_concentration_weight,
        );
        println!("Liquidity Risk: {:?}", liquidity_risk);
    }

    #[tokio::test]
    async fn test_calculate_sigma_apy() {
        let data = fetch_yield_and_utilization_rates().await.unwrap();
        println!(
            "Yields (APY in %) \nTotal: ({}) \nStart: {:?} \nEnd: {:?} \nValues: {}",
            data.yields_percent.len(),
            data.start,
            data.end,
            serde_json::to_string_pretty(&data.yields_percent).unwrap()
        );
        println!(
            "Utilization Rates (in %) \nTotal: ({}) \nStart: {:?} \nEnd: {:?} \nValues: {}",
            data.utilization_rates_percent.len(),
            data.start,
            data.end,
            serde_json::to_string_pretty(&data.utilization_rates_percent).unwrap()
        );
        let risk = calculate_lending_pool_risk(
            data.yields_percent,
            data.utilization_rates_percent,
            0.7,
            0.3,
        );
        println!("Risk: {:?}", risk);
    }
}
