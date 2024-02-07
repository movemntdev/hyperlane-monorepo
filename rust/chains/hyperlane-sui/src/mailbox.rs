use std::{collections::HashMap, num::NonZeroU64, ops::RangeInclusive, str::FromStr};

use async_trait::async_trait;
use base64::write;
use hyperlane_core::{
    ChainCommunicationError, ChainResult, ContractLocator, Decode, Encode, FixedPointNumber,
    HyperlaneAbi, HyperlaneChain, HyperlaneContract, HyperlaneDomain, HyperlaneMessage,
    HyperlaneProvider, Indexer, LogMeta, Mailbox, SequenceIndexer, TxCostEstimate, TxOutcome, H256,
    H512, U256,
};
use move_core_types::{identifier::Identifier, language_storage::StructTag};
use shared_crypto::intent::Intent;
use solana_sdk::pubkey::ParsePubkeyError;
use solana_sdk::signature::Keypair;
use sui_keys::keystore::{AccountKeystore, Keystore};
use sui_sdk::{
    json::{MoveTypeLayout, SuiJsonValue},
    rpc_types::{EventFilter, SuiTransactionBlockEffectsAPI},
    types::{
        base_types::{ObjectID, SuiAddress},
        execution, parse_sui_struct_tag,
        transaction::CallArg,
    },
};
use tracing::{info, instrument};
use url::Url;

use crate::{
    convert_hex_string_to_h256, convert_keypair_to_sui_keystore, get_filtered_events,
    move_mutate_call, move_view_call, send_owned_objects_request, total_gas, AddressFormatter,
    ConnectionConf, DispatchEventData, EventSourceLocator, ExecuteMode, FilterBuilder,
    GasPaymentEventData, MsgProcessEventData, Signer, SuiHpProvider, SuiRpcClient,
    TryIntoPrimitive, GAS_UNIT_PRICE,
};

/// A reference to a Mailbox contract on some Sui chain
pub struct SuiMailbox {
    pub(crate) domain: HyperlaneDomain,
    payer: Option<Signer>,
    pub(crate) sui_client: SuiRpcClient,
    pub(crate) packages_address: SuiAddress,
    rest_url: Url,
}

impl SuiMailbox {
    /// Create a new Sui Mailbox
    pub fn new(
        conf: &ConnectionConf,
        locator: ContractLocator,
        payer: Option<Signer>,
    ) -> ChainResult<Self> {
        let package_address = SuiAddress::from_bytes(<[u8; 32]>::from(locator.address)).unwrap();
        let sui_client = tokio::runtime::Runtime::new()
            .expect("Failed to create runtime")
            .block_on(async { SuiRpcClient::new().await })
            .expect("Failed to create SuiRpcClient");
        Ok(Self {
            domain: locator.domain.clone(),
            rest_url: conf.url.clone(),
            sui_client,
            packages_address: package_address,
            payer,
        })
    }

    /// Returns the package name in bytes from the chain give a `SuiAddress`
    async fn fetch_package_name(&self, package_address: &SuiAddress) -> ChainResult<Vec<u8>> {
        let view_response =
            send_owned_objects_request(&self.sui_client, package_address, "mailbox".to_string())
                .await?;
        let module_name = serde_json::from_str::<String>(&view_response).unwrap();
        //Not sure if module name is returned in hex format check this, unit test.
        let module_name_bytes =
            hex::decode(module_name.to_string().trim_start_matches("0x")).unwrap();
        Ok(module_name_bytes)
    }
}

impl HyperlaneContract for SuiMailbox {
    fn address(&self) -> H256 {
        self.packages_address.to_bytes().into()
    }
}

impl HyperlaneChain for SuiMailbox {
    fn domain(&self) -> &HyperlaneDomain {
        &self.domain
    }

    fn provider(&self) -> Box<dyn HyperlaneProvider> {
        let sui_provider = tokio::runtime::Runtime::new()
            .expect("Failed to create runtime")
            .block_on(async {
                SuiHpProvider::new(self.domain.clone(), self.rest_url.to_string().clone()).await
            });
        Box::new(sui_provider)
    }
}

impl std::fmt::Debug for SuiMailbox {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self as &dyn HyperlaneContract)
    }
}

#[async_trait]
impl Mailbox for SuiMailbox {
    #[instrument(err, ret, skip(self))]
    async fn count(&self, _maybe_lag: Option<NonZeroU64>) -> ChainResult<u32> {
        todo!() // need to implement Merkle Tree
    }

    #[instrument(err, ret, skip(self))]
    async fn delivered(&self, id: H256) -> ChainResult<bool> {
        let view_response = move_view_call(
            &self.sui_client,
            &self.packages_address,
            self.packages_address.clone(),
            "mailbox".to_string(),
            "delivered".to_string(),
            vec![],
            vec![CallArg::Pure(Vec::from(id.as_bytes()))],
        )
        .await?;
        let (bytes, type_tag) = &view_response[0].return_values[0];
        let delivered_json =
            SuiJsonValue::from_bcs_bytes(Some(&MoveTypeLayout::Bool), &bytes).unwrap();
        Ok(delivered_json
            .try_into_bool()
            .expect("Failed to convert to bool"))
    }

    #[instrument(err, ret, skip(self))]
    async fn default_ism(&self) -> ChainResult<H256> {
        let view_response = move_view_call(
            &self.sui_client,
            &self.packages_address,
            self.packages_address.clone(),
            "mailbox".to_string(),
            "get_default_ism".to_string(),
            vec![],
            vec![],
        )
        .await?;

        // @TODO this should be the zeroth index for both fields. But unit test this.
        let (bytes, type_tag) = &view_response[0].return_values[0];
        let ism_json =
            SuiJsonValue::from_bcs_bytes(Some(&MoveTypeLayout::Address), &bytes).unwrap();
        Ok(ism_json.try_into_h256().expect("Failed to convert to H256"))
    }

    #[instrument(err, ret, skip(self))]
    async fn recipient_ism(&self, id: H256) -> ChainResult<H256> {
        let view_response = move_view_call(
            &self.sui_client,
            &self.packages_address,
            self.packages_address.clone(),
            "mailbox".to_string(),
            "get_recipient_ism".to_string(),
            vec![],
            vec![CallArg::Pure(Vec::from(id.as_bytes()))],
        )
        .await?;

        // @TODO this should be the zeroth index for both fields. But unit test this.
        let (bytes, type_tag) = &view_response[0].return_values[0];
        let ism_json =
            SuiJsonValue::from_bcs_bytes(Some(&MoveTypeLayout::Address), &bytes).unwrap();
        Ok(ism_json.try_into_h256().expect("Failed to convert to H256"))
    }

    #[instrument(err, ret, skip(self))]
    async fn process(
        &self,
        message: &HyperlaneMessage,
        metadata: &[u8],
        _tx_gas_limit: Option<U256>,
    ) -> ChainResult<TxOutcome> {
        let recipient = SuiAddress::from_bytes(message.recipient.0).unwrap();
        let objects = self
            .sui_client
            .read_api()
            .get_owned_objects(recipient, None, None, None)
            .await
            .expect("Failed to get owned objects");
        let object = objects.data.first().expect("Failed to get owned objects");

        let mut encoded_message = vec![];
        message.write_to(&mut encoded_message).unwrap();

        let signer = self
            .payer
            .as_ref()
            .ok_or_else(|| ChainCommunicationError::SignerUnavailable)?;
        let recipient_module_name = self
            .fetch_package_name(&recipient)
            .await
            .expect("Failed to fetch package name");

        let response = move_mutate_call(
            &self.sui_client,
            signer,
            object.data.as_ref().unwrap().object_id, //check this not sure if this ID correlates to the module ID we want
            bcs::from_bytes(&recipient_module_name).unwrap(),
            "handle_message".to_string(),
            vec![],
            vec![
                SuiJsonValue::from_bcs_bytes(
                    Some(&MoveTypeLayout::U8),
                    &bcs::to_bytes(&recipient_module_name).unwrap(),
                )
                .expect("Failed to convert message to SuiJsonValue"),
                SuiJsonValue::from_bcs_bytes(
                    Some(&MoveTypeLayout::Vector(Box::new(MoveTypeLayout::U8))),
                    &bcs::to_bytes(&metadata.to_vec()).unwrap(),
                )
                .expect("Failed to convert metadata to SuiJsonValue"),
            ],
            ExecuteMode::LiveNetwork,
        )
        .await?;
        let tx_hash = convert_hex_string_to_h256(&response.digest.to_string()).unwrap();
        let has_success = response.confirmed_local_execution.unwrap();
        Ok(TxOutcome {
            transaction_id: H512::from(tx_hash),
            executed: has_success,
            gas_price: FixedPointNumber::from(GAS_UNIT_PRICE),
            gas_used: U256::from(total_gas(response)),
        })
    }

    #[instrument(err, ret, skip(self))]
    async fn process_estimate_costs(
        &self,
        message: &HyperlaneMessage,
        metadata: &[u8],
    ) -> ChainResult<TxCostEstimate> {
        let recipient = SuiAddress::from_bytes(message.recipient.0).unwrap();
        let objects = self
            .sui_client
            .read_api()
            .get_owned_objects(recipient, None, None, None)
            .await
            .expect("Failed to get owned objects");
        let object = objects.data.first().expect("Failed to get owned objects");

        let mut encoded_message = vec![];
        message.write_to(&mut encoded_message).unwrap();

        let recipient_module_name = self
            .fetch_package_name(&recipient)
            .await
            .expect("Failed to fetch package name");
        let signer = self
            .payer
            .as_ref()
            .ok_or_else(|| ChainCommunicationError::SignerUnavailable)?;
        let response = move_mutate_call(
            &self.sui_client,
            &signer,
            object.data.as_ref().unwrap().object_id, //check this not sure if this ID correlates to the module ID we want
            bcs::from_bytes(&recipient_module_name).unwrap(),
            "handle_message".to_string(),
            vec![],
            vec![
                SuiJsonValue::from_bcs_bytes(
                    Some(&MoveTypeLayout::U8),
                    &bcs::to_bytes(&recipient_module_name).unwrap(),
                )
                .expect("Failed to convert message to SuiJsonValue"),
                SuiJsonValue::from_bcs_bytes(
                    Some(&MoveTypeLayout::Vector(Box::new(MoveTypeLayout::U8))),
                    &bcs::to_bytes(&metadata.to_vec()).unwrap(),
                )
                .expect("Failed to convert metadata to SuiJsonValue"),
            ],
            ExecuteMode::Simulate,
        )
        .await
        .expect("Failed to execute transaction");
        Ok(TxCostEstimate {
            gas_limit: U256::from(total_gas(response)),
            gas_price: U256::from(GAS_UNIT_PRICE),
            l2_gas_limit: None,
        })
    }

    fn process_calldata(&self, _message: &HyperlaneMessage, _metadata: &[u8]) -> Vec<u8> {
        todo!()
    }
}

/// Struct that retrieves event data for a Sui Mailbox contract.
#[derive(Debug)]
pub struct SuiMailboxIndexer {
    mailbox: SuiMailbox,
    sui_client: SuiRpcClient,
    package: ObjectID,
    identifier: Identifier,
}

impl FilterBuilder for SuiMailboxIndexer {}

impl EventSourceLocator for SuiMailboxIndexer {
    fn package(&self) -> ObjectID {
        self.package
    }

    fn identifier(&self) -> Identifier {
        self.identifier.clone()
    }
}

impl SuiMailboxIndexer {
    /// Create a new SuiMailboxIndexer
    pub fn new(conf: &ConnectionConf, locator: ContractLocator) -> ChainResult<Self> {
        let sui_client = tokio::runtime::Runtime::new()
            .expect("Failed to create runtime")
            .block_on(async { SuiRpcClient::new().await })
            .expect("Failed to create SuiRpcClient");

        let hash_map = locator.modules.clone().unwrap();
        let package = hash_map.get("hp_mailbox").unwrap();
        let mailbox = SuiMailbox::new(conf, locator, None)?;

        Ok(Self {
            mailbox,
            sui_client,
            package: *package,
            identifier: Identifier::new("hp_mailbox").unwrap(),
        })
    }

    async fn get_finalized_block_number(&self) -> ChainResult<u32> {
        let chain_state = self
            .sui_client
            .read_api()
            .get_latest_checkpoint_sequence_number()
            .await
            .map_err(ChainCommunicationError::from_other)
            .unwrap();
        Ok(chain_state as u32)
    }
}

#[async_trait]
impl SequenceIndexer<HyperlaneMessage> for SuiMailboxIndexer {
    #[instrument(err, skip(self))]
    async fn sequence_and_tip(&self) -> ChainResult<(Option<u32>, u32)> {
        let tip = Indexer::<HyperlaneMessage>::get_finalized_block_number(self as _).await?;
        let count = Mailbox::count(&self.mailbox, None).await?;
        Ok((Some(count), tip))
    }
}

#[async_trait]
impl Indexer<HyperlaneMessage> for SuiMailboxIndexer {
    /// fetch the events from the mailbox Sui Move Module
    /// in Sui, we cannot filter range by blockheight,
    /// so we use the `range` arg to filter by timestamp
    async fn fetch_logs(
        &self,
        range: RangeInclusive<u32>,
    ) -> ChainResult<Vec<(HyperlaneMessage, LogMeta)>> {
        get_filtered_events::<HyperlaneMessage, GasPaymentEventData>(
            &self.sui_client,
            self.package(),
            self.identifier(),
            self.build_filter("DispatchEvent", range),
        )
        .await
    }

    async fn get_finalized_block_number(&self) -> ChainResult<u32> {
        self.get_finalized_block_number().await
    }
}

#[async_trait]
impl Indexer<H256> for SuiMailboxIndexer {
    async fn fetch_logs(&self, range: RangeInclusive<u32>) -> ChainResult<Vec<(H256, LogMeta)>> {
        get_filtered_events::<H256, GasPaymentEventData>(
            &self.sui_client,
            self.package(),
            self.identifier(),
            self.build_filter("ProcessEvent", range),
        )
        .await
    }

    async fn get_finalized_block_number(&self) -> ChainResult<u32> {
        self.get_finalized_block_number().await
    }
}

#[async_trait]
impl SequenceIndexer<H256> for SuiMailboxIndexer {
    async fn sequence_and_tip(&self) -> ChainResult<(Option<u32>, u32)> {
        info!("Message deliver indexing not implemented for Sui");
        let tip = Indexer::<H256>::get_finalized_block_number(self as _).await?;
        Ok((Some(1), tip))
    }
}

// TODO Don't support it for Sui
impl HyperlaneAbi for SuiMailboxIndexer {
    const SELECTOR_SIZE_BYTES: usize = 8;

    fn fn_map() -> HashMap<Vec<u8>, &'static str> {
        unimplemented!()
    }
}
