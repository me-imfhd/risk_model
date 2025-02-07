use deposit_conc::fetch_deposits;
use tracing::info;
use utilization_rate::get_total_borrows_and_supply;
use yield_data::fetch_yield_and_utilization_rates;

use crate::{
    liquidity_risk::{calculate_liquidity_risk, calculate_utilization_rate},
    risk_model::{
        get_seconds_until_next_hour, LiquidityRiskMetrics, ProtocolRisk, ProtocolRiskMetrics,
        RiskCalculationError, VolatilityRiskMetrics,
    },
    volatility_risk::calculate_lending_pool_risk,
};

mod deposit_conc;
mod utilization_rate;
mod yield_data;
pub struct KaminoRisk {
    pub redis_client: redis::Client,
}
use redis::AsyncCommands;

impl ProtocolRisk for KaminoRisk {
    const W_LIQ_D_CONC: f64 = 0.4;
    const W_LIQ_UTIL: f64 = 0.6;
    const W_VOL_APY: f64 = 0.7;
    const W_VOL_UTIL: f64 = 0.3;
    const W_LIQUIDITY: f64 = 0.4;
    const W_VOLATILITY: f64 = 0.3;
    const W_PROTOCOL: f64 = 0.3;
    fn redis_client(&self) -> &redis::Client {
        &self.redis_client
    }
    async fn calculate_liquidity_risk(&self) -> Result<LiquidityRiskMetrics, RiskCalculationError> {
        // Try to get cached deposit data
        let largest_deposit_key = "deposits:largest";
        let total_deposits_key = "deposits:total";

        let (largest_deposit, total_deposits) = if let (Ok(largest), Ok(total)) = (
            self.redis_get(largest_deposit_key).await,
            self.redis_get(total_deposits_key).await,
        ) {
            (
                largest
                    .parse::<u128>()
                    .map_err(|e| RiskCalculationError::ParseError(e.to_string()))?,
                total
                    .parse::<u128>()
                    .map_err(|e| RiskCalculationError::ParseError(e.to_string()))?,
            )
        } else {
            info!("Fetching deposits...");
            let deposits = fetch_deposits().await?;
            let largest = *deposits
                .iter()
                .max()
                .ok_or(RiskCalculationError::CustomError(
                    "No deposits found".to_string(),
                ))?;
            let total = deposits.iter().sum::<u128>();

            // Cache deposits data
            self.redis_set_until_next_hour(largest_deposit_key, &largest.to_string())
                .await?;
            self.redis_set_until_next_hour(total_deposits_key, &total.to_string())
                .await?;

            (largest, total)
        };

        // Try to get cached borrows and supply data
        let total_borrows_key = "utilization:total_borrows";
        let total_supply_key = "utilization:total_supply";

        let (total_borrows, total_supply) = if let (Ok(borrows), Ok(supply)) = (
            self.redis_get(total_borrows_key).await,
            self.redis_get(total_supply_key).await,
        ) {
            (
                borrows
                    .parse::<f64>()
                    .map_err(|e| RiskCalculationError::ParseError(e.to_string()))?,
                supply
                    .parse::<f64>()
                    .map_err(|e| RiskCalculationError::ParseError(e.to_string()))?,
            )
        } else {
            info!("Fetching borrows and supply...");
            let (borrows, supply) = get_total_borrows_and_supply().await?;

            // Cache borrows and supply data
            self.redis_set_until_next_hour(total_borrows_key, &borrows.to_string())
                .await?;
            self.redis_set_until_next_hour(total_supply_key, &supply.to_string())
                .await?;

            (borrows, supply)
        };

        // Calculate final values using cached data
        let deposit_concentration = (largest_deposit as f64) / (total_deposits as f64);
        let utilization_rate = calculate_utilization_rate(total_borrows, total_supply).ok_or(
            RiskCalculationError::CustomError("Total supply is 0".to_string()),
        )?;

        // Calculate final liquidity risk (not cached)
        info!("Calculating liquidity risk...");
        let liquidity_risk = calculate_liquidity_risk(
            deposit_concentration,
            utilization_rate,
            Self::W_LIQ_UTIL,
            Self::W_LIQ_D_CONC,
        );

        Ok(LiquidityRiskMetrics {
            total_borrows,
            total_supply,
            utilization_rate,
            largest_deposit,
            total_deposits,
            deposit_concentration,
            liquidity_risk,
        })
    }

    async fn calculate_volatility_risk(
        &self,
    ) -> Result<VolatilityRiskMetrics, RiskCalculationError> {
        // Try to get cached yield and utilization data
        let yields_key = "volatility:yields";
        let utilization_rates_key = "volatility:utilization_rates";

        let (yields_percent, utilization_rates_percent) = if let (Ok(yields), Ok(util_rates)) = (
            self.redis_get(yields_key).await,
            self.redis_get(utilization_rates_key).await,
        ) {
            (
                serde_json::from_str(&yields)
                    .map_err(|e| RiskCalculationError::ParseError(e.to_string()))?,
                serde_json::from_str(&util_rates)
                    .map_err(|e| RiskCalculationError::ParseError(e.to_string()))?,
            )
        } else {
            info!("Fetching yield and utilization rates...");
            let data = fetch_yield_and_utilization_rates().await?;

            // Cache the data
            self.redis_set_until_next_hour(
                yields_key,
                &serde_json::to_string(&data.yields_percent)
                    .map_err(|e| RiskCalculationError::ParseError(e.to_string()))?,
            )
            .await?;
            self.redis_set_until_next_hour(
                utilization_rates_key,
                &serde_json::to_string(&data.utilization_rates_percent)
                    .map_err(|e| RiskCalculationError::ParseError(e.to_string()))?,
            )
            .await?;

            (data.yields_percent, data.utilization_rates_percent)
        };

        // Calculate volatility risk using cached data (not cached)
        info!("Calculating volatility risk...");
        let volatility_risk = calculate_lending_pool_risk(
            yields_percent,
            utilization_rates_percent,
            Self::W_VOL_APY,
            Self::W_VOL_UTIL,
        )
        .ok_or(RiskCalculationError::CustomError(
            "Insufficient data".to_string(),
        ))?;

        Ok(VolatilityRiskMetrics {
            sigma_apy: volatility_risk.sigma_apy,
            sigma_utilization: volatility_risk.sigma_utilization,
            volatility_risk: volatility_risk.volatility_risk,
        })
    }

    async fn calculate_protocol_risk(&self) -> Result<ProtocolRiskMetrics, RiskCalculationError> {
        let mut connection = self
            .redis_client
            .get_multiplexed_async_connection()
            .await
            .map_err(|e| RiskCalculationError::RedisError(e))?;

        let cache_key = "protocol_risk";

        if let Ok(cached_result) = connection.get::<_, String>(cache_key).await {
            return Ok(ProtocolRiskMetrics {
                protocol_risk: cached_result
                    .parse::<f64>()
                    .map_err(|e| RiskCalculationError::ParseError(e.to_string()))?,
            });
        }

        // Constant protocol risk for Kamino
        let protocol_risk = 0.508;

        // Cache the result for 1 hour
        let _: () = connection
            .set_ex(
                cache_key,
                protocol_risk.to_string(),
                get_seconds_until_next_hour(),
            )
            .await
            .map_err(|e| RiskCalculationError::RedisError(e))?;

        Ok(ProtocolRiskMetrics { protocol_risk })
    }
}

#[cfg(test)]
mod kamino_tests {
    use super::{
        utilization_rate::get_total_borrows_and_supply,
        yield_data::fetch_yield_and_utilization_rates,
    };
    use crate::{
        kamino::deposit_conc::fetch_deposits,
        liquidity_risk::{
            calculate_concentration, calculate_liquidity_risk, calculate_utilization_rate,
        },
        volatility_risk::calculate_lending_pool_risk,
    };
    #[tokio::test]
    async fn test_liquidity_risk() {
        let utilization_weight = 0.6;
        let deposit_concentration_weight = 0.4;
        // Get deposit concentration
        let deposits = fetch_deposits().await.unwrap();
        let deposit_concentration = calculate_concentration(deposits).unwrap();
        tracing::info!("Deposit Concentration: {:?}", deposit_concentration);
        // Get utilization rate
        let (total_borrows, total_supply) = get_total_borrows_and_supply().await.unwrap();
        let utilization_rate = calculate_utilization_rate(total_borrows, total_supply).unwrap();
        tracing::info!("Utilization Rate: {:?}", utilization_rate);

        let liquidity_risk = calculate_liquidity_risk(
            deposit_concentration,
            utilization_rate,
            utilization_weight,
            deposit_concentration_weight,
        );
        tracing::info!("Liquidity Risk: {:?}", liquidity_risk);
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
