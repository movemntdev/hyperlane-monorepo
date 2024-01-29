use std::str::FromStr;

use hyperlane_core::{ChainCommunicationError, InterchainGasPayment, U256};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sui_sdk::{json::SuiJsonValue, rpc_types::SuiEvent, types::event::EventID};

use crate::convert_hex_string_to_h256;

pub trait TryIntoPrimitive {
    fn try_into_bool(&self) -> Result<bool, anyhow::Error>;
}

impl TryIntoPrimitive for SuiJsonValue {
    fn try_into_bool(&self) -> Result<bool, anyhow::Error> {
        match self.to_json_value() {
            Value::Bool(b) => Ok(b),
            _ => Err(anyhow::anyhow!("Failed to convert to bool")),
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
