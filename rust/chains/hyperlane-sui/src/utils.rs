use crate::{AddressFormatter, SuiRpcClient, TxSpecificData};
use hyperlane_core::{
    ChainCommunicationError, ChainResult, InterchainGasPayment, LogMeta, H256, H512, U256,
};
use solana_sdk::account;
use std::{ops::RangeInclusive, str::FromStr};
use sui_sdk::{
    rpc_types::{EventFilter, SuiEvent},
    types::{
        base_types::{ObjectID, SuiAddress},
        digests::TransactionDigest,
    },
};

/// Convert address string to H256
pub fn convert_hex_string_to_h256(addr: &str) -> Result<H256, String> {
    let formated_addr = format!("{:0>64}", addr.to_string().trim_start_matches("0x"));
    H256::from_str(&formated_addr).map_err(|e| e.to_string())
}

pub async fn get_filtered_events(
    sui_client: &SuiRpcClient,
    package_id: &Option<ObjectID>,
    struct_tag: &str,
    range: RangeInclusive<u32>,
) -> ChainResult<Vec<(InterchainGasPayment, LogMeta)>> {
    if package_id.is_none() {
        return Err(ChainCommunicationError::SuiObjectReadError(
            "Package ID is None".to_string(),
        ));
    }
    let events_page = sui_client
        .event_api()
        .query_events(EventFilter::Package(package_id.unwrap()), None, None, true)
        .await
        .map_err(|e| {
            ChainCommunicationError::SuiObjectReadError(format!(
                "Failed to query events from Sui: {}",
                e
            ))
        })?;

    //TODO: LogMeta will need to be an enum to handle different chains
    //the data its expecting here doesn't make much sense for Sui.
    let mut messages: Vec<(InterchainGasPayment, LogMeta)> = Vec::with_capacity((range.end() - range.start()) as usize);
    for event in events_page.data.into_iter() {
        //let json_data = event.parsed_json.clone();
        // Mainly using dummy values untile LogMeta is an enum
        let log_meta = LogMeta {
            address: event.sender.to_bytes().into(), // Should this be the sender?
            block_number: 0,                         // No block numbers in Sui
            block_hash: H256::zero(),                // No block hash in Sui
            transaction_id: H512::zero(),
            transaction_index: 0,    // Not sure what this val should be,
            log_index: U256::zero(), // No block structure in Sui
        };
        messages.push((event.parsed_json.try_into()?, log_meta));
    }
    Ok(messages)
}
