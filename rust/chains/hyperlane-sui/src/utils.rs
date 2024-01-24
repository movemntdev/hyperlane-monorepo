use crate::{AddressFormatter, SuiRpcClient, TxSpecificData};
use hyperlane_core::{ChainCommunicationError, ChainResult, LogMeta, H256, H512, U256};
use solana_sdk::account;
use sui_sdk::{types::{base_types::{ObjectID, SuiAddress}, digests::TransactionDigest}, rpc_types::{EventFilter, SuiEvent}};
use std::ops::RangeInclusive;

pub async fn get_filtered_events(
    sui_client: &SuiRpcClient,
    package_id: &Option<ObjectID>,
    struct_tag: &str,
    range: RangeInclusive<u64>,
) -> ChainResult<Vec<(SuiEvent, LogMeta)>> {
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
        .await?;

    //TODO: LogMeta will need to be an enum to handle different chains
    //the data its expecting here doesn't make much sense for Sui.
    let parsed_events: Vec<(SuiEvent, LogMeta)> = events_page
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
        todo!()
}
