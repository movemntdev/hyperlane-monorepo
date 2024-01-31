use std::str::FromStr;

use hyperlane_core::{ChainCommunicationError, Decode, HyperlaneMessage, InterchainGasPayment, H256, U256};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sui_sdk::{
    json::SuiJsonValue,
    rpc_types::{DryRunTransactionBlockResponse, SuiEvent, SuiTransactionBlockResponse},
    types::{base_types::{ObjectID, SuiAddress}, digests::TransactionDigest, event::{Event, EventID}, Identifier},
};

use crate::convert_hex_string_to_h256;

pub enum ExecuteMode {
    LiveNetwork,
    Simulate,
}

#[derive(Debug)]
pub struct SuiModule {
    pub package: ObjectID,
    pub module: Identifier
}

pub trait TryIntoPrimitive {
    fn try_into_bool(&self) -> Result<bool, anyhow::Error>;
    fn try_into_h256(&self) -> Result<H256, anyhow::Error>;
}

impl TryIntoPrimitive for SuiJsonValue {
    fn try_into_bool(&self) -> Result<bool, anyhow::Error> {
        match self.to_json_value() {
            Value::Bool(b) => Ok(b),
            _ => Err(anyhow::anyhow!("Failed to convert to bool")),
        }
    }

    fn try_into_h256(&self) -> Result<H256, anyhow::Error> {
        match self.to_json_value() {
            Value::String(s) => {
                // Improve Error handling here, get rid of expect
                Ok(convert_hex_string_to_h256(&s).expect("Failed to convert to H256"))
            }
            _ => Err(anyhow::anyhow!("Failed to convert to H256")),
        }
    }
}

pub trait ConvertFromDryRun {
    fn convert_from(dry_run_response: DryRunTransactionBlockResponse) -> Self;
}

impl ConvertFromDryRun for SuiTransactionBlockResponse {
    fn convert_from(dry_run_response: DryRunTransactionBlockResponse) -> Self {
        SuiTransactionBlockResponse {
            digest: TransactionDigest::default(),
            transaction: None, 
            raw_transaction: Vec::new(), 
            effects: Some(dry_run_response.effects),
            events: Some(dry_run_response.events),
            object_changes: Some(dry_run_response.object_changes),
            balance_changes: Some(dry_run_response.balance_changes),
            timestamp_ms: None, 
            confirmed_local_execution: None, 
            checkpoint: None, 
            errors: Vec::new(), 
            raw_effects: Vec::new(), 
        }
    }
}


/// Trait for event types which returns transaction_hash and block_height
pub trait TxSpecificData {
    /// returns block_height
    fn block_height(&self) -> String;
    /// returns transaction_hash
    fn transaction_hash(&self) -> String;
}

#[derive(Serialize, Deserialize, Debug, Clone)]
/// Move Value Data of GasPayment Event
pub struct GasPaymentEventData {
    /// dest domain the gas is paid for
    pub dest_domain: String,
    /// hyperlane message id.
    pub message_id: String,
    /// gas amount
    pub gas_amount: String,
    /// quoted gas payment
    pub required_amount: String,
    /// block number
    pub checkpoint_number: String,
    /// hash of transaction
    pub event_id: EventID,
}

impl TryFrom<Value> for GasPaymentEventData {
    type Error = ChainCommunicationError;
    fn try_from(event: serde_json::Value) -> Result<Self, Self::Error> {
        serde_json::from_str::<Self>(&event.to_string())
            .map_err(ChainCommunicationError::from_other)
    }
}

impl TryInto<InterchainGasPayment> for GasPaymentEventData {
    type Error = ChainCommunicationError;
    fn try_into(self) -> Result<InterchainGasPayment, Self::Error> {
        Ok(InterchainGasPayment {
            destination: self.dest_domain.parse::<u32>().unwrap(),
            message_id: convert_hex_string_to_h256(&self.message_id).unwrap(),
            payment: U256::from_str(&self.required_amount)
                .map_err(ChainCommunicationError::from_other)
                .unwrap(),
            gas_amount: U256::from_str(&self.gas_amount)
                .map_err(ChainCommunicationError::from_other)
                .unwrap(),
        })
    }
}

///Event Data of Message Dispatch
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DispatchEventData {
    pub dest_domain: u64,
    pub message: String,
    pub message_id: String,
    pub recipient: String,
    pub block_height: String,
    pub transaction_hash: String,
    pub sender: String,
}

impl TxSpecificData for DispatchEventData {
    fn block_height(&self) -> String {
        self.block_height.clone()
    }
    fn transaction_hash(&self) -> String {
        self.transaction_hash.clone()
    }
}

impl TryFrom<Event> for DispatchEventData {
    type Error = ChainCommunicationError;
    fn try_from(value: Event) -> Result<Self, Self::Error> {
        serde_json::from_str::<Self>(&value.data.to_string())
            .map_err(ChainCommunicationError::from_other)
    }
}

impl TryInto<HyperlaneMessage> for DispatchEventData {
    type Error = hyperlane_core::HyperlaneProtocolError;
    fn try_into(self) -> Result<HyperlaneMessage, Self::Error> {
        let hex_bytes = hex::decode(&self.message.trim_start_matches("0x")).unwrap();
        HyperlaneMessage::read_from(&mut &hex_bytes[..])
    }
}

/// Move Calue Data of GasPaymen Event
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MsgProcessEventData {
    /// hyperlane message id
    pub message_id: String,
    /// domain of origin chain
    pub origin_domain: u32,
    /// address of sender (router)
    pub sender: String,
    /// address of recipient
    pub recipient: String,
    /// block number
    pub block_height: String,
    /// has of transaction
    pub transaction_hash: String,
}