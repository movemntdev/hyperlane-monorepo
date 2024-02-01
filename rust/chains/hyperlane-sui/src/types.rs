use std::{ops::RangeInclusive, str::FromStr};

use hyperlane_core::{
    accumulator::{incremental::IncrementalMerkle, TREE_DEPTH}, ChainCommunicationError, Decode, HyperlaneMessage, InterchainGasPayment, H256, U256
};
use move_core_types::language_storage::StructTag;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sui_sdk::{
    json::SuiJsonValue,
    rpc_types::{
        DryRunTransactionBlockResponse, EventFilter, SuiEvent, SuiTransactionBlockResponse,
    },
    types::{
        base_types::{ObjectID, SuiAddress},
        digests::TransactionDigest,
        event::{Event, EventEnvelope, EventID},
        Identifier,
    },
};

use crate::convert_hex_string_to_h256;

pub enum ExecuteMode {
    LiveNetwork,
    Simulate,
}

#[derive(Debug)]
pub struct SuiModule {
    pub package: ObjectID,
    pub module: Identifier,
}

pub trait TryIntoPrimitive {
    fn try_into_bool(&self) -> Result<bool, anyhow::Error>;
    fn try_into_h256(&self) -> Result<H256, anyhow::Error>;
    fn try_into_merkle_tree(&self) -> Result<IncrementalMerkle, anyhow::Error>;
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

    // important to write exsaustive unit tests for this
    fn try_into_merkle_tree(&self) -> Result<IncrementalMerkle, anyhow::Error> {
        let json_value = self.to_json_value();

        if let Value::Object(map) = json_value {
            let branch = map.get("branch").ok_or_else(|| {
                anyhow::anyhow!("Failed to get branch from merkle tree json value")
            })?;
            let count = map.get("count").ok_or_else(|| {
                anyhow::anyhow!("Failed to get count from merkle tree json value")
            })?;

            let branch_vec = branch.as_array().ok_or_else(|| {
                anyhow::anyhow!("Failed to get branch as array from merkle tree json value")
            })?;
            let mut branch_array: [H256; TREE_DEPTH] = [H256::default(); TREE_DEPTH];
            for (i, vec_u8) in branch_vec.iter().enumerate() {
                let vec_u8 = vec_u8.as_array().ok_or_else(|| {
                    anyhow::anyhow!("Failed to get branch as array from merkle tree json value")
                })?;
                let bytes: Vec<u8> = vec_u8
                    .iter()
                    .map(|u8| u8.as_u64().unwrap() as u8)
                    .collect();
                branch_array[1] = H256::from_slice(&bytes);
            }

            let count_usize = usize::try_from(count.as_u64().unwrap()).map_err(|_| {
                anyhow::anyhow!("Failed to get count as usize from merkle tree json value")
            })?;

            Ok(IncrementalMerkle {
                branch: branch_array,
                count: count_usize,
            })
        } else {
            Err(anyhow::anyhow!("Failed to convert to IncrementalMerkle"))
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
    /// returns the checkopoint number. Blockheight does not exist in Sui.
    fn checkpoint(&self) -> String;
    /// returns transaction digest
    fn tx_digest(&self) -> String;
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

impl From<GasPaymentEventData> for HyperlaneMessage {
    fn from(event_data: GasPaymentEventData) -> Self {
        HyperlaneMessage {
            version: 3,
            nonce: 0,
            origin: 0,
            sender: H256::zero(),
            destination: event_data.dest_domain.parse::<u32>().unwrap(),
            recipient: H256::zero(),
            body: Vec::new(),
        }
    }
}

impl From<GasPaymentEventData> for H256 {
    fn from(event_data: GasPaymentEventData) -> Self {
        convert_hex_string_to_h256(&event_data.message_id).unwrap()
    }
}

impl TryFrom<SuiEvent> for GasPaymentEventData {
    type Error = ChainCommunicationError;
    fn try_from(event: SuiEvent) -> Result<Self, Self::Error> {
        let contents = bcs::from_bytes::<GasPaymentEventData>(&event.bcs)
            .map_err(ChainCommunicationError::from_other)
            .unwrap();
        Ok(contents)
    }
}

impl From<GasPaymentEventData> for InterchainGasPayment {
    fn from(event_data: GasPaymentEventData) -> Self {
        InterchainGasPayment {
            destination: event_data.dest_domain.parse::<u32>().unwrap(),
            message_id: convert_hex_string_to_h256(&event_data.message_id).unwrap(),
            payment: U256::from_str(&event_data.required_amount).unwrap(),
            gas_amount: U256::from_str(&event_data.gas_amount).unwrap(),
        }
    }
}

impl TxSpecificData for GasPaymentEventData {
    fn checkpoint(&self) -> String {
        self.checkpoint_number.to_string()
    }
    fn tx_digest(&self) -> String {
        self.event_id.tx_digest.to_string()
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
    pub transaction_digest: String,
    pub sender: String,
}

impl TxSpecificData for DispatchEventData {
    fn checkpoint(&self) -> String {
        self.block_height.clone()
    }
    fn tx_digest(&self) -> String {
        self.transaction_digest.clone()
    }
}

impl TryFrom<SuiEvent> for DispatchEventData {
    type Error = ChainCommunicationError;
    fn try_from(value: SuiEvent) -> Result<Self, Self::Error> {
        let contents = bcs::from_bytes::<DispatchEventData>(&value.bcs)
            .map_err(ChainCommunicationError::from_other)
            .unwrap();
        Ok(contents)
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

pub trait EventSourceLocator {
    fn package_address(&self) -> SuiAddress;
    fn module(&self) -> &SuiModule;
}

pub trait FilterBuilder: EventSourceLocator {
    /// Build a filter for the event
    fn build_filter(&self, event_name: &str, range: RangeInclusive<u32>) -> EventFilter {
        EventFilter::All(vec![
            EventFilter::Sender(self.package_address()),
            EventFilter::MoveEventModule {
                package: self.module().package,
                module: self.module().module.clone(),
            },
            EventFilter::TimeRange {
                start_time: *range.start() as u64,
                end_time: *range.end() as u64,
            },
            EventFilter::MoveEventType(StructTag {
                address: self.package_address().into(),
                module: self.module().module.clone(),
                name: Identifier::new(event_name).expect("Failed to create Identifier"),
                type_params: vec![],
            }),
        ])
    }
}
