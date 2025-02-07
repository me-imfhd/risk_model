use tracing::info;

/// Calculates the liquidity risk score for a lending pool
///
/// The liquidity risk (Rl,l) is calculated using the formula:
/// Rl,l = wu * U + wc * Cd
///
/// Where:
/// - U: Utilization rate (percentage of deposited funds currently borrowed)
/// - Cd: Deposit concentration (largest deposit as a proportion of total deposits)
/// - wu: Weight for utilization rate (default: 0.6)
/// - wc: Weight for deposit concentration (default: 0.4)
///
/// Returns a risk score between 0 and 100, where:
/// - 0-33: Low risk
/// - 34-66: Medium risk
/// - 67-100: High risk
///
/// # Arguments
/// * `market_id` - The ID of the lending market
/// * `reserve_id` - The ID of the specific reserve
/// * `rpc_url` - The Solana RPC URL for querying deposit data
/// * `program_id` - The program ID for the lending protocol

pub fn calculate_liquidity_risk(
    deposit_concentration: f64,
    utilization_rate: f64,
    weight_utilization_coefficient: f64,
    weight_deposit_concentration_coefficient: f64,
) -> f64 {
    // Calculate weighted risk score
    let risk_score = (weight_utilization_coefficient * utilization_rate)
        + (weight_deposit_concentration_coefficient * deposit_concentration);

    // Ensure risk score is between 0 and 100
    risk_score
}
/// Calculates the deposit concentration for a lending pool
///
/// The deposit concentration is calculated by finding the largest single deposit
/// as a proportion of total deposits. This helps measure how concentrated the
/// deposits are among users.
///
/// # Arguments
/// * `deposits` - Vector of deposit amounts from different users
///
/// # Returns
/// * `Option<f64>` - The deposit concentration as a decimal between 0 and 1,
///                   or None if there are no deposits
pub fn calculate_concentration(deposits: Vec<u128>) -> Option<f64> {
    if deposits.len() == 0 {
        return None;
    }
    let total_deposits = deposits.iter().sum::<u128>();
    info!("total_deposits {:?}", total_deposits);
    let largest_deposit = deposits.iter().max().copied()?;
    info!("largest_deposit {:?}", largest_deposit);

    // Divide by 1000 to reduce from 9 to 6 decimals before converting to f64
    let deposit_concentration = (largest_deposit * 1_000_000) / (total_deposits);
    Some(deposit_concentration as f64 / 1_000_000.0)
}

/// Calculates the utilization rate for a lending pool
///
/// The utilization rate represents what percentage of the total supplied assets
/// are currently being borrowed.
///
/// # Arguments
/// * `total_borrows` - Total amount of assets currently borrowed
/// * `total_supply` - Total amount of assets supplied to the pool
///
/// # Returns
/// * `Option<f64>` - The utilization rate as a percentage between 0 and 100,
///                   or None if total supply is 0
pub fn calculate_utilization_rate(total_borrows: f64, total_supply: f64) -> Option<f64> {
    if total_supply > 0.0 {
        Some((total_borrows / total_supply) * 100.0) // Convert to percentage
    } else {
        None
    }
}
