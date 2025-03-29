use std::collections::HashMap;
use std::fmt::{self, Display};
use std::time::{Duration, SystemTime};

use solana_sdk::pubkey::Pubkey;

use crate::risk_model::{Protocol, RiskProfile};

/// Represents a pool where funds can be allocated
#[derive(Debug, Clone, PartialEq)]
pub struct Pool {
    pub id: Protocol,
    pub balance: u64,
}

/// Portfolio for a single user containing multiple risk profiles
#[derive(Debug, Clone, PartialEq)]
pub struct UserPortfolio {
    pub user_wallet: Pubkey,
    pub risk_profiles: HashMap<RiskProfile, ProfileAllocation>,
    pub last_rebalance: SystemTime,
}

/// Display a basis point value as a percentage string
fn format_basis_points(basis_points: u64) -> String {
    let whole_percent = basis_points / 100;
    let decimal = (basis_points % 100) / 10; // First decimal place
    format!("{}.{}%", whole_percent, decimal)
}

impl Display for UserPortfolio {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(
            f,
            "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
        )?;
        writeln!(f, "💼 USER PORTFOLIO | Wallet: {}", self.user_wallet)?;
        writeln!(
            f,
            "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
        )?;

        if self.risk_profiles.is_empty() {
            writeln!(f, "📝 No risk profiles found in portfolio")?;
        } else {
            let mut total_value = 0;

            // Calculate overall total
            for allocation in self.risk_profiles.values() {
                total_value = total_value + allocation.total_amount;
            }

            writeln!(f, "📊 TOTAL VALUE | {}", format_amount(total_value))?;
            writeln!(f, "⏰ LAST REBALANCE | {:?}", self.last_rebalance)?;
            writeln!(f, "\n📋 RISK PROFILES")?;

            for (risk_profile, allocation) in &self.risk_profiles {
                // Calculate percentage in basis points (10000 = 100%)
                let percentage_bps = if total_value > 0 {
                    // Scale up first to avoid precision loss
                    (allocation.total_amount as u128)
                        .saturating_mul(10_000)
                        .saturating_div(total_value as u128) as u64
                } else {
                    0
                };

                writeln!(
                    f,
                    "\n🔹 {} | {} ({} of portfolio)",
                    risk_profile,
                    format_amount(allocation.total_amount),
                    format_basis_points(percentage_bps)
                )?;

                writeln!(f, "  Protocol   | Amount        | Allocation")?;
                writeln!(f, "  -----------|---------------|-------------")?;

                for (protocol, amount) in &allocation.pool_allocations {
                    // Calculate protocol percentage in basis points
                    let protocol_bps = if allocation.total_amount > 0 {
                        (*amount as u128)
                            .saturating_mul(10_000)
                            .saturating_div(allocation.total_amount as u128)
                            as u64
                    } else {
                        0
                    };

                    writeln!(
                        f,
                        "  {} | {:12} | {}",
                        protocol,
                        format_amount(*amount),
                        format_basis_points(protocol_bps)
                    )?;
                }
            }
        }

        writeln!(
            f,
            "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
        )?;
        Ok(())
    }
}

// Add a standalone format_amount function for use in Display implementation
fn format_amount(amount: u64) -> String {
    if amount >= 1_000_000_000 {
        format!("{:.2}B", amount as f64 / 1_000_000_000.0)
    } else if amount >= 1_000_000 {
        format!("{:.2}M", amount as f64 / 1_000_000.0)
    } else if amount >= 1_000 {
        format!("{:.2}K", amount as f64 / 1_000.0)
    } else {
        format!("{}", amount)
    }
}

/// Allocation for a specific risk profile
#[derive(Debug, Clone, PartialEq)]
pub struct ProfileAllocation {
    pub risk_profile: RiskProfile,
    pub pool_allocations: HashMap<Protocol, u64>, // Pool ID -> Amount
    pub total_amount: u64,
}

impl Display for ProfileAllocation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(
            f,
            "📊 PROFILE ALLOCATION | {} | Total: {}",
            self.risk_profile,
            format_amount(self.total_amount)
        )?;

        if self.pool_allocations.is_empty() {
            writeln!(f, "  No allocations")?;
        } else {
            writeln!(f, "  Protocol   | Amount        | Allocation")?;
            writeln!(f, "  -----------|---------------|-------------")?;

            for (protocol, amount) in &self.pool_allocations {
                // Calculate protocol percentage in basis points
                let protocol_bps = if self.total_amount > 0 {
                    (*amount as u128)
                        .saturating_mul(10_000)
                        .saturating_div(self.total_amount as u128) as u64
                } else {
                    0
                };

                writeln!(
                    f,
                    "  {} | {:12} | {}",
                    protocol,
                    format_amount(*amount),
                    format_basis_points(protocol_bps)
                )?;
            }
        }

        Ok(())
    }
}

/// System A: AI Risk Model interface
pub trait RiskWeightModel {
    /// Get recommended pool weights for a given risk profile
    fn get_recommended_weights(&self, profile: &RiskProfile) -> HashMap<Protocol, u64>;
}

/// Rebalancing system that connects risk model with transaction execution
pub struct RebalancingSystem<R: RiskWeightModel> {
    pub risk_model: R,
    pub rebalance_interval: Duration,
}

pub trait RebalanceSystem<R: RiskWeightModel> {
    fn new(risk_model: R) -> RebalancingSystem<R> {
        println!("📊 SYSTEM INIT | Creating new rebalancing system with 1 hour interval");
        RebalancingSystem {
            risk_model,
            rebalance_interval: Duration::from_secs(1 * 60 * 60), // 1 hour
        }
    }
    fn should_rebalance(&self, portfolio: &UserPortfolio) -> bool;
    fn rebalance(&mut self, portfolio: &mut UserPortfolio) -> Result<(), String>;
    fn deposit(
        &mut self,
        portfolio: &mut UserPortfolio,
        profile: RiskProfile,
        amount: u64,
    ) -> Result<TransactionSystemDeposits, String>;
    fn withdraw(
        &mut self,
        portfolio: &mut UserPortfolio,
        profile: &RiskProfile,
        amount: u64,
    ) -> Result<(), String>;
}

/// Response from the transaction system API containing deposits that need to be executed
pub struct TransactionSystemDeposits {
    /// List of deposits that need to be processed by the transaction system
    pub deposits_to_execute: Vec<DepositToExecute>,
}
impl Display for TransactionSystemDeposits {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(
            f,
            "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
        )?;
        writeln!(f, "💰 DEPOSITS TO EXECUTE")?;
        for deposit in &self.deposits_to_execute {
            writeln!(f, "{}", deposit)?;
        }
        writeln!(
            f,
            "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
        )?;
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct DepositToExecute {
    pub protocol: Protocol,
    pub amount: u64,
    pub allocation_basis_points: u64,
}

impl Display for DepositToExecute {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} | {} | {} allocation",
            self.protocol,
            format_amount(self.amount),
            format_basis_points(self.allocation_basis_points)
        )
    }
}

impl<R: RiskWeightModel> RebalanceSystem<R> for RebalancingSystem<R> {
    /// Deposit funds into a risk profile
    fn deposit(
        &mut self,
        portfolio: &mut UserPortfolio,
        profile: RiskProfile,
        amount: u64,
    ) -> Result<TransactionSystemDeposits, String> {
        let weights = self.risk_model.get_recommended_weights(&profile);

        // Create or update profile allocation
        let profile_allocation = portfolio
            .risk_profiles
            .entry(profile.clone())
            .or_insert_with(|| ProfileAllocation {
                risk_profile: profile.clone(),
                pool_allocations: HashMap::new(),
                total_amount: 0,
            });

        // Add amount to total
        profile_allocation.total_amount = profile_allocation.total_amount.saturating_add(amount);

        // Allocate funds according to weights and prepare deposits
        let mut deposits_to_execute = Vec::new();
        for (pool_id, basis_points) in weights {
            // Calculate allocation amount (scaled to maintain precision)
            let allocation_amount = (amount as u128)
                .saturating_mul(basis_points as u128)
                .saturating_div(10_000) as u64;

            // Update pool allocation
            *profile_allocation
                .pool_allocations
                .entry(pool_id.clone())
                .or_insert(0) = profile_allocation
                .pool_allocations
                .get(&pool_id)
                .unwrap_or(&0)
                .saturating_add(allocation_amount);

            deposits_to_execute.push(DepositToExecute {
                protocol: pool_id,
                amount: allocation_amount,
                allocation_basis_points: basis_points,
            });
        }

        Ok(TransactionSystemDeposits {
            deposits_to_execute,
        })
    }

    /// Check if rebalancing is needed for a portfolio
    fn should_rebalance(&self, portfolio: &UserPortfolio) -> bool {
        let time_since_last = SystemTime::now()
            .duration_since(portfolio.last_rebalance)
            .unwrap_or(Duration::from_secs(0));

        let should_rebalance = time_since_last >= self.rebalance_interval;

        should_rebalance
    }

    /// Rebalance a user's portfolio
    fn rebalance(&mut self, portfolio: &mut UserPortfolio) -> Result<(), String> {
        println!(
            "\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
        );
        println!("🔄 REBALANCE | Starting portfolio rebalance");

        for (profile, allocation) in &mut portfolio.risk_profiles {
            println!(
                "\n📊 REBALANCING PROFILE | {} | Total: {}",
                profile,
                format_amount(allocation.total_amount)
            );
            self.rebalance_profile(profile, allocation)?;
        }

        // Update last rebalance time
        portfolio.last_rebalance = SystemTime::now();
        println!(
            "\n✅ REBALANCE COMPLETE | New rebalance time: {:?}",
            portfolio.last_rebalance
        );
        println!(
            "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n"
        );

        Ok(())
    }

    /// Rebalance a specific risk profile
    fn rebalance_profile(
        &mut self,
        profile: &RiskProfile,
        allocation: &mut ProfileAllocation,
    ) -> Result<(), String> {
        // Get recommended weights from risk model (in basis points)
        let target_weights = self.risk_model.get_recommended_weights(profile);

        // Calculate target amounts
        let mut target_amounts = HashMap::new();
        let mut current_amounts = HashMap::new();

        for (pool_id, basis_points) in &target_weights {
            // Calculate target amount (scaled to maintain precision)
            let target_amount = (allocation.total_amount as u128)
                .saturating_mul(*basis_points as u128)
                .saturating_div(10_000) as u64;

            target_amounts.insert(pool_id.clone(), target_amount);

            // Store current amount
            let current_amount = *allocation.pool_allocations.get(pool_id).unwrap_or(&0);
            current_amounts.insert(pool_id.clone(), current_amount);
        }

        // Calculate deltas between current and target allocations
        let mut deltas = HashMap::new();
        for (pool_id, target_amount) in &target_amounts {
            let current_amount = *current_amounts.get(pool_id).unwrap_or(&0);

            // Calculate delta (can be negative)
            let delta = match target_amount.checked_sub(current_amount) {
                Some(positive_delta) => positive_delta as i64,
                None => -(current_amount as i64 - *target_amount as i64),
            };

            deltas.insert(pool_id.clone(), delta);
        }

        // Execute transfers to rebalance
        let mut positive_deltas: Vec<_> = deltas.iter().filter(|(_, delta)| **delta > 0).collect();
        let mut negative_deltas: Vec<_> = deltas.iter().filter(|(_, delta)| **delta < 0).collect();

        // Sort by absolute delta value
        positive_deltas.sort_by(|a, b| b.1.cmp(a.1));
        negative_deltas.sort_by(|a, b| a.1.cmp(b.1)); // Most negative first

        let mut transfers = Vec::new();

        // Execute transfers
        for (to_pool, positive_delta) in positive_deltas {
            let mut remaining_delta = *positive_delta;

            for (from_pool, negative_delta) in negative_deltas.clone() {
                if remaining_delta <= 0 || *negative_delta >= 0 {
                    continue;
                }

                let transfer_amount =
                    std::cmp::min(remaining_delta as u64, negative_delta.abs() as u64);

                if transfer_amount > 0 {
                    transfers.push((from_pool.clone(), to_pool.clone(), transfer_amount));

                    // Update allocations
                    *allocation
                        .pool_allocations
                        .entry(to_pool.clone())
                        .or_insert(0) = allocation
                        .pool_allocations
                        .get(to_pool)
                        .unwrap_or(&0)
                        .saturating_add(transfer_amount);

                    *allocation
                        .pool_allocations
                        .entry(from_pool.clone())
                        .or_insert(0) = allocation
                        .pool_allocations
                        .get(from_pool)
                        .unwrap_or(&0)
                        .saturating_sub(transfer_amount);

                    // Update remaining delta
                    remaining_delta = remaining_delta.saturating_sub(transfer_amount as i64);
                }

                if remaining_delta <= 0 {
                    break;
                }
            }
        }

        println!("🔄 REBALANCE OPERATION | {}", profile);

        // Display target weights
        println!("\n📈 TARGET WEIGHTS");
        for (protocol, weight) in target_weights {
            println!("    {}: {}", protocol, format_basis_points(weight));
        }

        // Display allocation changes
        println!("\n📊 ALLOCATION CHANGES");
        println!("Protocol   | Current       | Target        | Change");
        println!("-----------+---------------+---------------+---------------");

        for (pool_id, target_amount) in &target_amounts {
            let current_amount = *current_amounts.get(pool_id).unwrap_or(&0);
            let delta = if let Some(d) = deltas.get(pool_id) {
                *d
            } else {
                0
            };

            // Format for display
            let change_symbol = if delta > 0 {
                "+"
            } else if delta < 0 {
                "-"
            } else {
                " "
            };
            let abs_delta = delta.abs() as u64;

            // Calculate change percentage in basis points
            let change_bps = if current_amount > 0 {
                ((abs_delta as u128)
                    .saturating_mul(10_000)
                    .saturating_div(current_amount as u128)) as u64
            } else {
                10_000 // 100% change if no current amount
            };

            println!(
                "{} | {:12} | {:12} | {}{} ({})",
                pool_id,
                format_amount(current_amount),
                format_amount(*target_amount),
                change_symbol,
                format_amount(abs_delta),
                format_basis_points(change_bps)
            );
        }

        // Display transfers
        if !transfers.is_empty() {
            println!("\n🔄 TRANSFERS");
            for (from_pool, to_pool, amount) in &transfers {
                println!(
                    "    {} ➡️ {} | Amount: {}",
                    from_pool,
                    to_pool,
                    format_amount(*amount)
                );
            }
        } else {
            println!("\n✅ NO TRANSFERS NEEDED");
        }

        Ok(())
    }

    /// Withdraw funds from a risk profile
    fn withdraw(
        &mut self,
        portfolio: &mut UserPortfolio,
        profile: &RiskProfile,
        amount: u64,
    ) -> Result<(), String> {
        let profile_allocation = match portfolio.risk_profiles.get_mut(profile) {
            Some(allocation) => allocation,
            None => return Err(format!("Risk profile not found in portfolio")),
        };

        if amount > profile_allocation.total_amount {
            println!(
                "❌ WITHDRAWAL FAILED | Insufficient funds | Requested: {} | Available: {}",
                format_amount(amount),
                format_amount(profile_allocation.total_amount)
            );
            return Err(format!("Insufficient funds for withdrawal"));
        }

        // Calculate proportion to withdraw from each pool (in basis points)
        let proportion_bps = (amount as u128)
            .saturating_mul(10_000)
            .saturating_div(profile_allocation.total_amount as u128)
            as u64;

        let mut withdrawals = Vec::new();

        for (pool_id, pool_amount) in &profile_allocation.pool_allocations {
            // Calculate withdrawal amount (scaled for precision)
            let withdrawal_amount = (*pool_amount as u128)
                .saturating_mul(proportion_bps as u128)
                .saturating_div(10_000) as u64;

            let remaining = pool_amount.saturating_sub(withdrawal_amount);
            withdrawals.push((pool_id.clone(), withdrawal_amount, remaining));
        }

        // Execute withdrawals
        for (pool_id, withdrawal_amount, remaining) in &withdrawals {
            // Update pool allocation
            if let Some(pool_amount) = profile_allocation.pool_allocations.get_mut(pool_id) {
                *pool_amount = *remaining;
            }
        }

        // Update total amount
        profile_allocation.total_amount = profile_allocation.total_amount.saturating_sub(amount);

        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        println!(
            "💸 WITHDRAW | Amount: {} | Risk Profile: {}",
            format_amount(amount),
            profile
        );

        println!(
            "\n📊 WITHDRAWAL PROPORTION | {} of total holdings",
            format_basis_points(proportion_bps)
        );

        println!("\n🔄 WITHDRAWING FROM POOLS");
        println!("    Protocol   | Amount        | Remaining");
        println!("    -----------|---------------|---------------");

        for (protocol, amount, remaining) in &withdrawals {
            println!(
                "    {} | {:12} | {}",
                protocol,
                format_amount(*amount),
                format_amount(*remaining)
            );
        }

        println!(
            "\n💼 PORTFOLIO | Updated total amount: {}",
            format_amount(profile_allocation.total_amount)
        );
        println!("✅ WITHDRAWAL COMPLETE");
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

        Ok(())
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    // Mock implementation of RiskWeightModel
    struct MockRiskModel;

    impl RiskWeightModel for MockRiskModel {
        fn get_recommended_weights(&self, profile: &RiskProfile) -> HashMap<Protocol, u64> {
            let mut weights = HashMap::new();
            match profile {
                RiskProfile::Low => {
                    weights.insert(Protocol::Kamino, 10000);
                }
                RiskProfile::Medium => {
                    // Initial weights from the example
                    let (drift_weight, kamino_weight) = if rand::random() {
                        (4000, 6000)
                    } else {
                        (6000, 4000)
                    };
                    weights.insert(Protocol::Drift, drift_weight);
                    weights.insert(Protocol::Kamino, kamino_weight);
                }
                RiskProfile::High => {
                    let (drift_weight, kamino_weight) = if rand::random() {
                        (3000, 5000)
                    } else {
                        (5000, 3000)
                    };
                    weights.insert(Protocol::Kamino, kamino_weight);
                    weights.insert(Protocol::Drift, drift_weight);
                    weights.insert(Protocol::Marginfy, 1000);
                    weights.insert(Protocol::Solend, 1000);
                }
            }
            let sum: u64 = weights.values().sum();
            assert_eq!(sum, 10000, "Sum of weights must equal 10000");
            weights
        }
    }

    #[test]
    fn rebalancing_system_test() {
        let mut rebalancing_system = RebalancingSystem::new(MockRiskModel);
        let mut portfolio = UserPortfolio {
            user_wallet: Pubkey::default(),
            risk_profiles: HashMap::new(),
            last_rebalance: SystemTime::now(),
        };
        println!("{}", portfolio);
        let deposits_to_execute = rebalancing_system
            .deposit(&mut portfolio, RiskProfile::High, 1_000_000_000)
            .unwrap();
        println!("{}", deposits_to_execute);
        println!("{}", portfolio);

        std::thread::sleep(Duration::from_secs(10));

        let result = rebalancing_system.rebalance(&mut portfolio).unwrap();
        println!("{}", portfolio);
        std::thread::sleep(Duration::from_secs(10));
        let result = rebalancing_system.rebalance(&mut portfolio).unwrap();
        println!("{}", portfolio);
        std::thread::sleep(Duration::from_secs(10));
        let result = rebalancing_system.rebalance(&mut portfolio).unwrap();
        println!("{}", portfolio);
    }
    #[test]
    fn test_deposit() {
        // We would implement a test for deposit here
    }

    #[test]
    fn test_rebalance() {
        // We would implement a test for rebalance here
    }

    #[test]
    fn test_withdraw() {
        // We would implement a test for withdraw here
    }
}
