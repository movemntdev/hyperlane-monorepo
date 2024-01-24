use crate::{AddressFormatter, SuiRpcClient, TxSpecificData};
use hyperlane_core::{ChainCommunicationError, ChainResult, LogMeta, H256, H512, U256};
use solana_sdk::account;
use sui_sdk::{types::{base_types::{ObjectID, SuiAddress}, digests::TransactionDigest}, rpc_types::{EventFilter, SuiEvent}};
use std::{ops::RangeInclusive, str::FromStr};

/// Convert address string to H256
pub fn convert_hex_string_to_h256(addr: &str) -> Result<H256, String> {
    let formated_addr = format!("{:0>64}", addr.to_string().trim_start_matches("0x"));
    H256::from_str(&formated_addr).map_err(|e| e.to_string())
}

pub async fn get_filtered_events<T>(
    sui_client: &SuiRpcClient,
    package_id: &Option<ObjectID>,
    struct_tag: &str,
    range: RangeInclusive<u64>,
) -> ChainResult<Vec<(T, LogMeta)>> 
{

    if package_id.is_none() {
        return Err(ChainCommunicationError::SuiObjectReadError(
            "Package ID is None".to_string(),
        ));
    }
    let events_page = sui_client
        .event_api()
        .query_events(
            EventFilter::Package(package_id.unwrap()),
            None,
            None,
            true,
        )
        .await
        .map_err(|e| {
            ChainCommunicationError::SuiObjectReadError(format!(
                "Failed to query events from Sui: {}",
                e
            ))
        })?;

    //TODO: LogMeta will need to be an enum to handle different chains
    //the data its expecting here doesn't make much sense for Sui.
    let parsed_events: Vec<(T, LogMeta)> = events_page
        .data
        .into_iter()
        .map(|event| {
            // Mainly using dummy values untile LogMeta is an enum
            let log_meta = LogMeta {
                address: event.sender.to_bytes().into(), // Should this be the sender? 
                block_number:  0, // No block numbers in Sui
                block_hash: H256::zero(), // No block hash in Sui
                transaction_id: H512::zero(),  
                transaction_index: 0, // Not sure what this val should be,
                log_index: U256::zero(), // No block structure in Sui 
            };
            (event, log_meta)
        })
        .collect();
    Ok(parsed_events)
}
