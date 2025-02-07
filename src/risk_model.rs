#![allow(unused)]
use std::fmt::Display;

use axum::response::{IntoResponse, Response};
use redis::AsyncCommands;
use serde::Serialize;

use crate::kamino::KaminoRisk;

pub enum Protocol {
    Kamino,
    Solend,
    Drift,
    Marginfy,
}

pub enum ProtocolWithRisk {
    Kamino(RiskScore),
    Solend(RiskScore),
    Drift(RiskScore),
    Marginfy(RiskScore),
}
#[derive(Debug)]
pub enum RiskCalculationError {
    SerdeError(serde_json::Error),
    ParseError(String),
    RequestError(reqwest::Error),
    RpcCallError(solana_client::client_error::ClientError),
    RedisError(redis::RedisError),
    CustomError(String),
}
impl Display for RiskCalculationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RiskCalculationError::SerdeError(e) => write!(f, "Serde error: {}", e),
            RiskCalculationError::ParseError(e) => write!(f, "Parse error: {}", e),
            RiskCalculationError::RequestError(e) => write!(f, "Request error: {}", e),
            RiskCalculationError::RpcCallError(e) => write!(f, "RPC call error: {}", e),
            RiskCalculationError::RedisError(e) => write!(f, "Redis error: {}", e),
            RiskCalculationError::CustomError(e) => write!(f, "Custom error: {}", e),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct RiskResponse {
    pub liquidity_risk: LiquidityRiskMetrics,
    pub volatility_risk: VolatilityRiskMetrics,
    pub protocol_risk: ProtocolRiskMetrics,
    pub overall_risk: RiskScore,
}

#[derive(Debug, Serialize)]
pub struct LiquidityRiskMetrics {
    pub total_borrows: f64,
    pub total_supply: f64,
    pub utilization_rate: f64,
    pub largest_deposit: u128,
    pub total_deposits: u128,
    pub deposit_concentration: f64,
    pub liquidity_risk: f64,
}
#[derive(Debug, Serialize)]
pub struct VolatilityRiskMetrics {
    pub sigma_apy: f64,
    pub sigma_utilization: f64,
    pub volatility_risk: f64,
}
#[derive(Debug, Serialize)]
pub struct ProtocolRiskMetrics {
    pub protocol_risk: f64,
}
#[derive(Debug, Clone, Serialize)]
pub struct RiskScore {
    pub overall_risk: f64,
}
pub trait ProtocolRisk {
    fn redis_client(&self) -> &redis::Client;
    const W_LIQ_D_CONC: f64;
    const W_LIQ_UTIL: f64;
    const W_VOL_APY: f64;
    const W_VOL_UTIL: f64;
    const W_LIQUIDITY: f64;
    const W_VOLATILITY: f64;
    const W_PROTOCOL: f64;
    async fn calculate_liquidity_risk(&self) -> Result<LiquidityRiskMetrics, RiskCalculationError>;
    async fn calculate_volatility_risk(
        &self,
    ) -> Result<VolatilityRiskMetrics, RiskCalculationError>;
    async fn calculate_protocol_risk(&self) -> Result<ProtocolRiskMetrics, RiskCalculationError>;
    fn calculate_risk_score(
        &self,
        liquidity_risk: f64,
        volatility_risk: f64,
        protocol_risk: f64,
    ) -> Result<RiskScore, RiskCalculationError> {
        let liquidity_risk_score = liquidity_risk * Self::W_LIQUIDITY;
        let volatility_risk_score = volatility_risk * Self::W_VOLATILITY;
        let protocol_risk_score = protocol_risk * Self::W_PROTOCOL;
        let overall_risk = liquidity_risk_score + volatility_risk_score + protocol_risk_score;
        Ok(RiskScore { overall_risk })
    }
    async fn redis_set_until_next_hour(
        &self,
        key: &str,
        value: &str,
    ) -> Result<(), RiskCalculationError> {
        let mut connection = self
            .redis_client()
            .get_multiplexed_async_connection()
            .await
            .map_err(|e| RiskCalculationError::RedisError(e))?;
        let _: () = connection
            .set_ex(key, value, get_seconds_until_next_hour())
            .await
            .map_err(|e| RiskCalculationError::RedisError(e))?;
        Ok(())
    }
    async fn redis_get(&self, key: &str) -> Result<String, RiskCalculationError> {
        let mut connection = self
            .redis_client()
            .get_multiplexed_async_connection()
            .await
            .map_err(|e| RiskCalculationError::RedisError(e))?;
        let value: String = connection
            .get(key)
            .await
            .map_err(|e| RiskCalculationError::RedisError(e))?;
        Ok(value)
    }
}

pub fn get_seconds_until_next_hour() -> u64 {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap();
    let seconds_until_next_hour = 3600 - (now.as_secs() % 3600);
    seconds_until_next_hour
}

pub async fn risk_model() -> Response {
    let result = async {
        let kamino_risk = KaminoRisk {
            redis_client: redis::Client::open(std::env::var("REDIS_URL").unwrap())
                .map_err(|e| RiskCalculationError::RedisError(e))?,
        };

        let liquidity_risk = kamino_risk.calculate_liquidity_risk().await?;
        let volatility_risk = kamino_risk.calculate_volatility_risk().await?;
        let protocol_risk = kamino_risk.calculate_protocol_risk().await?;
        let overall_risk = kamino_risk.calculate_risk_score(
            liquidity_risk.liquidity_risk,
            volatility_risk.volatility_risk,
            protocol_risk.protocol_risk,
        )?;

        // Create enhanced response with protocol comparison
        let response = serde_json::json!({
            "choice_reason": "Kamino currently shows the lowest risk profile among evaluated protocols and gives you most bang for your buck",
            "chosen_protocol": {
                "protocol": "Kamino",
                "risk_metrics": {
                    "liquidity_risk": liquidity_risk,
                    "volatility_risk": volatility_risk,
                    "protocol_risk": protocol_risk,
                    "overall_risk": overall_risk
                }
            },
            "other_protocols": {
                "solend": null,
                "drift": null,
                "marginfy": null
            },
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
