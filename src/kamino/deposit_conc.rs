use anchor_client::solana_sdk::pubkey::Pubkey;
use serde::Deserialize;
use solana_account_decoder::UiDataSliceConfig;
use solana_client::{
    rpc_config::{RpcAccountInfoConfig, RpcProgramAccountsConfig},
    rpc_filter::{Memcmp, MemcmpEncodedBytes, RpcFilterType},
};
use std::str::FromStr;

use crate::risk_model::RiskCalculationError;

pub async fn fetch_deposits() -> Result<Vec<u128>, RiskCalculationError> {
    let rpc_url = format!(
        "https://mainnet.helius-rpc.com?api-key={}",
        std::env::var("HELIUS_API_KEY").expect("HELIUS_API_KEY must be set")
    );
    let program_id = "KLend2g3cP87fffoy8q1mQqGKjrxjC8boSyAYavgmjD";
    let client = solana_client::nonblocking::rpc_client::RpcClient::new(rpc_url.to_string());
    // First get all account public keys without data

    let fetched_accounts: Vec<Pubkey> = client
        .get_program_accounts_with_config(
            &Pubkey::from_str(program_id)
                .map_err(|e| RiskCalculationError::ParseError(e.to_string()))?,
            RpcProgramAccountsConfig {
                filters: Some(vec![
                    RpcFilterType::DataSize(3336 + 8),
                    RpcFilterType::Memcmp(Memcmp::new(
                        0,
                        MemcmpEncodedBytes::Bytes(vec![168, 206, 141, 106, 88, 76, 172, 167]),
                    )),
                ]),
                account_config: RpcAccountInfoConfig {
                    encoding: None,
                    data_slice: Some(UiDataSliceConfig {
                        offset: 0,
                        length: 8,
                    }),
                    commitment: None,
                    min_context_slot: None,
                },
                with_context: None,
            },
        )
        .await
        .map_err(|e| RiskCalculationError::RpcCallError(e))?
        .into_iter()
        .map(|(pk, _)| pk)
        .collect();
    // println!("Total Accounts {:?}", accounts.len());

    // Process accounts in chunks
    const CHUNK_SIZE: usize = 100;
    let futures = fetched_accounts
        .chunks(CHUNK_SIZE)
        .map(|chunk| {
            let pubkeys: Vec<Pubkey> = chunk.to_vec();
            let rpc_url = rpc_url.to_string();
            tokio::spawn(async move {
                let client = solana_client::nonblocking::rpc_client::RpcClient::new(rpc_url);
                let account_infos = client
                    .get_multiple_accounts_with_config(
                        &pubkeys,
                        RpcAccountInfoConfig {
                            data_slice: Some(UiDataSliceConfig {
                                offset: 88 + 8,
                                length: 1088,
                            }),
                            encoding: None,
                            commitment: None,
                            min_context_slot: None,
                        },
                    )
                    .await?;
                let mut chunk_deposits = Vec::new();
                for mut account_info in account_infos.value.into_iter().flatten() {
                    [168, 206, 141, 106, 88, 76, 172, 167]
                        .iter()
                        .enumerate()
                        .for_each(|(i, &byte)| account_info.data[i] = byte);
                    let obligation: Obligation = match account_info.deserialize_data() {
                        Err(err) => {
                            tracing::error!("Error while deserializing obligation: {}", err);
                            continue;
                        }
                        Ok(data) => data,
                    };
                    let user_total_deposits = obligation
                        .deposits
                        .iter()
                        .filter(|collateral| collateral.deposit_reserve != Pubkey::default())
                        .map(|collateral| collateral.deposited_amount as u128)
                        .fold(0u128, |acc, amount| acc.saturating_add(amount));

                    if user_total_deposits > 0 {
                        chunk_deposits.push(user_total_deposits);
                    }
                }
                Ok::<Vec<u128>, solana_client::client_error::ClientError>(chunk_deposits)
            })
        })
        .collect::<Vec<_>>();

    let mut deposits_by_user = Vec::new();
    let mut total_deposits: u128 = 0;
    let mut error_count = 0;
    for handle in futures {
        match handle
            .await
            .map_err(|e| RiskCalculationError::CustomError(e.to_string()))?
        {
            Ok(chunk_deposits) => {
                deposits_by_user.extend(chunk_deposits.clone());
                for deposit in chunk_deposits {
                    total_deposits = total_deposits.saturating_add(deposit);
                }
            }
            Err(e) => {
                tracing::error!("Error: {}", e);
                error_count += 1;
            }
        }
    }

    tracing::info!("error_count {:?}", error_count);
    tracing::info!("success_count {:?}", fetched_accounts.len() - error_count);
    Ok(deposits_by_user)
}

#[derive(Debug, Default, Deserialize)]
struct Obligation {
    pub deposits: [ObligationCollateral; 8],
}
#[allow(unused)]
#[derive(Debug, Default, Deserialize)]
struct ObligationCollateral {
    pub deposit_reserve: Pubkey,
    pub deposited_amount: u64,
    pub market_value_sf: u128,
    pub borrowed_amount_against_this_collateral_in_elevation_group: u64,
    pub padding: [u64; 9],
}

#[cfg(test)]
mod tests {
    use crate::liquidity_risk::calculate_concentration;

    use super::*;
    // Example usage
    #[tokio::test]
    async fn test() {
        match fetch_deposits().await {
            Ok(deposits) => {
                let deposit_concentration = calculate_concentration(deposits)
                    .ok_or(RiskCalculationError::CustomError(
                        "No deposits found".to_string(),
                    ))
                    .unwrap();
                println!("Deposit Concentration: {:?}", deposit_concentration)
            }
            Err(e) => eprintln!("Error calculating deposit concentration: {:?}", e),
        }
    }
}
