use crate::{
    AddressFormatter, ConvertFromDryRun, ExecuteMode, GasPaymentEventData, HyperlaneSuiError,
    SuiModule, SuiRpcClient, TxSpecificData,
};
use anyhow::{Chain, Error};
use fastcrypto::encoding::Encoding;
use fastcrypto::hash::HashFunction;
use hyperlane_core::{
    ChainCommunicationError, ChainResult, InterchainGasPayment, LogMeta, H256, H512, U256,
};
use serde_json::Value;
use shared_crypto::intent::{Intent, IntentMessage};
use solana_sdk::{account, signature::Keypair};
use std::{ops::RangeInclusive, str::FromStr};
use sui_config::{
    sui_config_dir, Config, PersistedConfig, SUI_CLIENT_CONFIG, SUI_KEYSTORE_FILENAME,
};
use sui_keys::keystore::{AccountKeystore, FileBasedKeystore, Keystore};
use sui_sdk::{
    json::SuiJsonValue,
    rpc_types::{
        DevInspectResults, EventFilter, SuiEvent, SuiExecutionResult, SuiParsedData,
        SuiTransactionBlockEffectsAPI, SuiTransactionBlockResponse,
        SuiTransactionBlockResponseOptions, SuiTypeTag,
    },
    sui_client_config::{SuiClientConfig, SuiEnv},
    types::crypto::SignatureScheme::ED25519,
    types::{
        base_types::{MoveObjectType, ObjectID, SuiAddress},
        digests::TransactionDigest,
        object::MoveObject,
        programmable_transaction_builder::ProgrammableTransactionBuilder,
        quorum_driver_types::ExecuteTransactionRequestType,
        transaction::{
            Argument, CallArg, Command, ProgrammableMoveCall, ProgrammableTransaction, Transaction,
            TransactionData, TransactionKind,
        },
        Identifier, TypeTag,
    },
    types::{crypto::DefaultHash, event::Event},
    wallet_context::WalletContext,
};
use tracing::info;

pub const GAS_UNIT_PRICE: u64 = 100; //check this is correct

/// Convert address string to H256
pub fn convert_hex_string_to_h256(addr: &str) -> Result<H256, String> {
    let formated_addr = format!("{:0>64}", addr.to_string().trim_start_matches("0x"));
    H256::from_str(&formated_addr).map_err(|e| e.to_string())
}

// TODO: Check this fn
pub async fn get_filtered_events<T, S>(
    sui_client: &SuiRpcClient,
    module: &SuiModule,
    filter: EventFilter,
) -> ChainResult<Vec<(T, LogMeta)>>
where
    S: TryFrom<SuiEvent> + TxSpecificData + TryInto<T> + Clone,
    ChainCommunicationError: From<<S as TryFrom<SuiEvent>>::Error> + From<<S as TryInto<T>>::Error>,
{
    let events_page = sui_client
        .event_api()
        .query_events(filter, None, None, true)
        .await
        .map_err(|e| {
            ChainCommunicationError::SuiObjectReadError(format!(
                "Failed to query events from Sui: {}",
                e
            ))
        })?;

    let mut messages: Vec<(T, LogMeta)> = Vec::new();
    for event in events_page.data.into_iter() {
        let tx = sui_client
            .read_api()
            .get_transaction_with_options(
                event.id.tx_digest,
                SuiTransactionBlockResponseOptions::full_content(),
            )
            .await
            .expect("Failed to get transaction");
        let log_meta = LogMeta {
            address: event.sender.to_bytes().into(), // Should this be the sender?
            block_number: 0,                         // No block numbers in Sui
            block_hash: H256::zero(),                // No block hash in Sui
            transaction_id: H512::zero(), // fix dummy for now
            transaction_index: 0,    // Not sure what this val should be,
            log_index: U256::zero(), // No block structure in Sui
        };
        let gas_payment_event_data: S = event.try_into()?;
        messages.push((gas_payment_event_data.try_into()?, log_meta));
    }
    Ok(messages)
}

/// Attempts to get the module name from the chain
pub async fn send_owned_objects_request(
    sui_client: &SuiRpcClient,
    package_address: &SuiAddress,
    module_name: String,
) -> ChainResult<String> {
    // Attempt to get the owned objects from the client
    let response = sui_client
        .read_api()
        .get_owned_objects(*package_address, None, None, Some(1))
        .await
        .map_err(ChainCommunicationError::from_other)?;

    // Extract the first item's data if available
    let first_item_data = response
        .data
        .first()
        .and_then(|item| item.data.as_ref())
        .and_then(|data| data.content.clone())
        .ok_or_else(|| ChainCommunicationError::SuiObjectReadError("No data found".to_string()))?;

    // Match against the parsed data
    match first_item_data {
        SuiParsedData::Package(pkg) => {
            // Attempt to find the module name in the disassembled package keys
            let module_name_key = pkg
                .disassembled
                .keys()
                .find(|&k| k == &module_name)
                .ok_or_else(|| {
                    ChainCommunicationError::SuiObjectReadError(format!(
                        "Module '{}' not found in package",
                        module_name
                    ))
                })?;
            Ok(module_name_key.to_string())
        }
        // Handle other cases or unimplemented data types
        _ => Err(ChainCommunicationError::SuiObjectReadError(
            "Unexpected data type".to_string(),
        )),
    }
}

/// Make a call to a move view only public function.
/// Internally, the ProgrammableTransactionBuilder
/// will validate inputs and error if invalid args ar passed.
// TODO: package_address and sender are the same value? Or are they?
pub async fn move_view_call(
    sui_client: &SuiRpcClient,
    sender: &SuiAddress,
    package_address: SuiAddress,
    module_name: String,
    function: String,
    type_args: Vec<SuiTypeTag>,
    args: Vec<CallArg>,
) -> ChainResult<Vec<SuiExecutionResult>> {
    let type_args = type_args
        .into_iter()
        .map(|tag| tag.try_into().expect("Invalid type tag"))
        .collect::<Vec<TypeTag>>();
    let mut ptb = ProgrammableTransactionBuilder::new();
    let move_call = ptb
        .move_call(
            ObjectID::from_address(package_address.into()),
            Identifier::new(module_name).expect("Invalid module name"),
            Identifier::new(function).expect("Invalid function name"),
            type_args,
            args,
        )
        .expect("Failed to build move call");
    let tx = TransactionKind::ProgrammableTransaction(ptb.finish());
    let inspect = sui_client
        .read_api()
        .dev_inspect_transaction_block(*sender, tx, None, None, None)
        .await
        .expect("Failed to get transaction block");
    if let Some(execution_results) = inspect.results {
        Ok(execution_results)
    } else {
        return Err(ChainCommunicationError::SuiObjectReadError(
            "No execution results found".to_string(),
        ));
    }
}

pub async fn move_mutate_call(
    sui_client: &SuiRpcClient,
    payer_keystore: FileBasedKeystore,
    package_id: ObjectID,
    module_name: String,
    function_name: String,
    type_args: Vec<SuiTypeTag>,
    args: Vec<SuiJsonValue>,
    execute: ExecuteMode,
) -> ChainResult<SuiTransactionBlockResponse> {
    let signer_account = payer_keystore.addresses()[0];
    let call = sui_client
        .transaction_builder()
        .move_call(
            signer_account,
            package_id,
            &module_name,
            &function_name,
            type_args,
            args,
            None, // The node will pick a gas object belonging to the singer if none is provided.
            1000,
        )
        .await
        .expect("Failed to build move call");
    let signature = payer_keystore
        .sign_secure(&signer_account, &call, Intent::sui_transaction())
        .expect("Failed to sign message");

    let response: SuiTransactionBlockResponse = match execute {
        ExecuteMode::LiveNetwork => sui_client
            .quorum_driver_api()
            .execute_transaction_block(
                Transaction::from_data(call, vec![signature]),
                SuiTransactionBlockResponseOptions::full_content(),
                Some(ExecuteTransactionRequestType::WaitForLocalExecution),
            )
            .await
            .unwrap(),
        ExecuteMode::Simulate => {
            let response = sui_client
                .read_api()
                .dry_run_transaction_block(call)
                .await
                .unwrap();
            SuiTransactionBlockResponse::convert_from(response)
        }
    };

    match response.confirmed_local_execution {
        Some(true) => Ok(response),
        _ => Err(ChainCommunicationError::SuiObjectReadError(
            "Failed to execute transaction".to_string(),
        )),
    }
}

pub async fn convert_keypair_to_sui_keystore(
    sui_client: &SuiRpcClient,
    payer: &Keypair,
) -> Result<FileBasedKeystore, anyhow::Error> {
    let wallet_conf = sui_config_dir()?.join(SUI_CLIENT_CONFIG);
    let keystore_path = sui_config_dir()?.join(SUI_KEYSTORE_FILENAME);

    // check if a wallet exists and if not, create a wallet and a sui client config
    if !keystore_path.exists() {
        let keystore = FileBasedKeystore::new(&keystore_path)?;
        keystore.save()?;
    }

    if !wallet_conf.exists() {
        let keystore = FileBasedKeystore::new(&keystore_path)?;
        let mut client_config = SuiClientConfig::new(keystore.into());

        client_config.add_env(SuiEnv::testnet());
        client_config.add_env(SuiEnv::devnet());
        client_config.add_env(SuiEnv::localnet());

        if client_config.active_env.is_none() {
            client_config.active_env = client_config.envs.first().map(|env| env.alias.clone());
        }

        client_config.save(&wallet_conf)?;
        info!("Client config file is stored in {:?}.", &wallet_conf);
    }

    let mut keystore = FileBasedKeystore::new(&keystore_path)?;
    let mut client_config: SuiClientConfig = PersistedConfig::read(&wallet_conf)?;

    let default_active_address = if let Some(address) = keystore.addresses().first() {
        *address
    } else {
        keystore
            .generate_and_add_new_key(ED25519, None, None, None)?
            .0
    };

    if keystore.addresses().len() < 2 {
        keystore.generate_and_add_new_key(ED25519, None, None, None)?;
    }

    client_config.active_address = Some(default_active_address);
    client_config.save(&wallet_conf)?;

    Ok(keystore)
}

/// Get the gas total payment events from the chain
pub fn total_gas(response: SuiTransactionBlockResponse) -> u64 {
    let effects = response.effects.unwrap();
    let gas_summary = effects.gas_cost_summary();
    gas_summary.computation_cost + gas_summary.storage_cost + gas_summary.non_refundable_storage_fee
}
