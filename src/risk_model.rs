#![allow(unused)]
use std::fmt::Display;

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
pub struct RiskScore {
    pub liquidity_risk: f64,
    pub volatility_risk: f64,
    pub protocol_risk: f64,
}
pub trait ProtocolRisk {
    async fn calculate_liquidity_risk(
        &self,
        weight_deposit_concentration_coefficient: f64,
        weight_utilization_coefficient: f64,
    ) -> Result<f64, RiskCalculationError>;
    async fn calculate_volatility_risk(
        &self,
        weight_apy_coefficient: f64,
        weight_utilization_coefficient: f64,
    ) -> Result<f64, RiskCalculationError>;
    async fn calculate_protocol_risk(&self) -> Result<f64, RiskCalculationError>;
}

pub fn get_seconds_until_next_hour() -> u64 {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap();
    let seconds_until_next_hour = 3600 - (now.as_secs() % 3600);
    seconds_until_next_hour
}
