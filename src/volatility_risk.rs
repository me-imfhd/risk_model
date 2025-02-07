#![allow(unused)]
use chrono::{DateTime, Timelike, Utc};
use serde::Deserialize;
use std::error::Error;

/// Calculates the combined lending pool risk based on APY and utilization rate volatilities
///
/// # Formula
/// Rv,l = w_a * σ_APY + w_u * σ_U
/// where:
/// - Rv,l is the total volatility risk for lending pools
/// - w_a is the weight coefficient for APY volatility (default: 0.7)
/// - w_u is the weight coefficient for utilization rate volatility (default: 0.3)
/// - σ_APY is the annualized APY volatility
/// - σ_U is the annualized utilization rate volatility
///
/// # Parameters
/// * `yields` - Vector of historical APY values over the last 24 hours
/// * `utilization_rates` - Vector of historical utilization rates over the last 24 hours
/// * `w_a` - Weight coefficient for APY volatility (optional, defaults to 0.7)
/// * `w_u` - Weight coefficient for utilization rate volatility (optional, defaults to 0.3)
///
/// # Returns
/// Returns the combined lending pool risk as a f64, or None if calculations fail
pub fn calculate_lending_pool_risk(
    yields: Vec<f64>,
    utilization_rates: Vec<f64>,
    weight_apy_coefficient: f64,
    weight_utilization_coefficient: f64,
) -> Option<f64> {
    let sigma_apy = calculate_sigma_apy(yields)?;
    let sigma_util = calculate_sigma_utilization(utilization_rates)?;

    Some(weight_apy_coefficient * sigma_apy + weight_utilization_coefficient * sigma_util)
}

/// Calculates the annualized volatility (sigma) of APY values
///
/// # Formula
/// σ = √(1/24 * ∑(APY_i - APY_avg)²)
/// where:
/// - σ (sigma) represents the annualized volatility
/// - APY_i is the current APY value
/// - APY_avg is the average of historical APY values
/// - The factor 1/24 is used to annualize the daily volatility
///
/// # Parameters
/// * `yields` - Vector of historical APY values over the last 24 hours
///
/// # Returns
/// Returns the annualized volatility as a f64
fn calculate_sigma_apy(yields: Vec<f64>) -> Option<f64> {
    let n = yields.len() as f64;
    if n < 2.0 {
        // Need at least 2 points to calculate volatility
        return None;
    }

    let avg_apy = yields.iter().sum::<f64>() / n;

    let sum_squared_diff: f64 = yields
        .iter()
        .map(|&apy_i| (apy_i - avg_apy).powi(2))
        .sum::<f64>();

    // Calculate annualized volatility (sigma)
    // The factor 1/24 is used to annualize the daily volatility
    Some((sum_squared_diff / 24.0).sqrt())
}

/// Calculates the annualized volatility (sigma) of utilization rates
///
/// # Formula
/// σ_U = √(1/24 * ∑(U_i - U_avg)²)
/// where:
/// - σ_U represents the annualized volatility of utilization rates
/// - U_i is the current utilization rate
/// - U_avg is the average of historical utilization rates
/// - The factor 1/24 is used to annualize the daily volatility
///
/// # Parameters
/// * `utilization_rates` - Vector of historical utilization rates over the last 24 hours
///
/// # Returns
/// Returns the annualized volatility as a f64
fn calculate_sigma_utilization(utilization_rates: Vec<f64>) -> Option<f64> {
    let n = utilization_rates.len() as f64;
    if n < 2.0 {
        // Need at least 2 points to calculate volatility
        return None;
    }

    let avg_utilization = utilization_rates.iter().sum::<f64>() / n;

    let sum_squared_diff: f64 = utilization_rates
        .iter()
        .map(|&util_i| (util_i - avg_utilization).powi(2))
        .sum::<f64>();

    // Calculate annualized volatility (sigma)
    // The factor 1/24 is used to annualize the daily volatility
    Some((sum_squared_diff / 24.0).sqrt())
}
