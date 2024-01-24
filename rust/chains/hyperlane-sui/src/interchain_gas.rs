
use std::ops::RangeInclusive;

use async_trait::async_trait;
use hyperlane_core::{
    ChainCommunicationError, ChainResult, ContractLocator, HyperlaneChain, HyperlaneContract, HyperlaneDomain, HyperlaneProvider, Indexer, InterchainGasPaymaster, InterchainGasPayment, LogMeta, H256
};
use sui_sdk::types::{base_types::ObjectID, digests::TransactionDigest};
use tracing::{info, instrument};
use hex;
use crate::{get_filtered_events, ConnectionConf, SuiHpProvider, SuiRpcClient};
use::sui_sdk::types::base_types::SuiAddress;

/// Format an address to bytes and hex literal. 
pub trait AddressFormatter {
    /// Convert an address to bytes.
    fn to_bytes(&self) -> [u8; 32];
    /// Convert an address to hex literal.
    fn to_hex_literal(&self) -> String;
}

impl AddressFormatter for SuiAddress {
    fn to_bytes(&self) -> [u8; 32] {
        let mut bytes = [0u8; 32];
        bytes.copy_from_slice(self.to_vec().as_slice());
        bytes
    }

    fn to_hex_literal(&self) -> String {
        format!("0x{}", hex::encode(self.to_vec()))
    }
}

/// A reference to an TGP contract on Sui Chain.
#[derive(Debug)]
pub struct SuiInterchainGasPaymaster {
    domain: HyperlaneDomain,
    package_address: SuiAddress,
    rest_url: String,
}

impl SuiInterchainGasPaymaster {
    /// Create a new Sui IGP.
    pub fn new(conf: &ConnectionConf, locator: &ContractLocator) -> Self {
        let package_address = 
            SuiAddress::from_bytes(<[u8; 32]>::from(locator.address)).unwrap();
        Self {
            domain: locator.domain.clone(),
            rest_url: conf.url.to_string(),
            package_address,
        }
    }
}

impl HyperlaneContract for SuiInterchainGasPaymaster {
    fn address(&self) -> H256 {
        self.package_address.to_bytes().into()
    }
}

impl HyperlaneChain for SuiInterchainGasPaymaster {
    fn domain(&self) -> &HyperlaneDomain {
        &self.domain
    }

    fn provider(&self) -> Box<dyn HyperlaneProvider> {
       let sui_provider = tokio::runtime::Runtime::new()
            .expect("Failed to create runtime")
            .block_on(async {
                SuiHpProvider::new(self.domain.clone(), self.rest_url.clone()).await
            }).expect("Failed to create SuiHpProvider");
        Box::new(sui_provider) 
    }
}

impl InterchainGasPaymaster for SuiInterchainGasPaymaster {}

/// Struct that retrieves event data for a Sui IGP contract.
#[derive(Debug)]
pub struct SuiInterchainGasPaymasterIndexer {
    sui_client: SuiRpcClient,
    package_address: SuiAddress,
    package_id: Option<ObjectID>,
}

impl SuiInterchainGasPaymasterIndexer {
    /// Create a new Sui IGP indexer.
    pub fn new(conf: &ConnectionConf, locator: ContractLocator) -> Self {
        let package_address = 
            SuiAddress::from_bytes(<[u8; 32]>::from(locator.address)).unwrap();
        let sui_client = tokio::runtime::Runtime::new()
            .expect("Failed to create runtime")
            .block_on(async {
                SuiRpcClient::new(conf.url.to_string()).await
            }).expect("Failed to create SuiRpcClient");
        let owned_objects = tokio::runtime::Runtime::new()
            .expect("Failed to create runtime")
            .block_on(async {
                sui_client
                    .read_api()
                    .get_owned_objects(package_address, None, None, None)
                    .await
                    .expect("Failed to get owned objects")
            });
        let object = owned_objects
            .data
            .first()
            .unwrap_or_else(|| panic!("No owned objects found for package address: {}", package_address.to_hex_literal()));
        if let Some(data) = &object.data {
            return Self {
                sui_client,
                package_address,
                package_id: Some(data.object_id)
            };
        } else {
           Self {
                sui_client,
                package_address,
                package_id: None, 
            } 
        }
         
    }
}

#[async_trait]
impl Indexer<InterchainGasPayment> for SuiInterchainGasPaymasterIndexer {
    #[instrument(err, skip(self))]
    async fn fetch_logs(
        &self,
        range: RangeInclusive<u32>,
    ) -> ChainResult<Vec<(InterchainGasPayment, LogMeta)>> {
        get_filtered_events(
            &self.sui_client,
            &self.package_id,
            &format!("{}::igps::IgpState", self.package_address.to_hex_literal()),
            range,
        ).await?
    }

    /// Sui is a DAG-based blockchain and uses checkpoints for node 
    /// synchronization and global transaction ordering. So this method when 
    /// implemented for `SuiInterchainGasPaymasterIndexer` will return the
    /// latest checkpoint sequence number.
    #[instrument(level = "debug", err, ret, skip(self))]
    async fn get_finalized_block_number(&self) -> ChainResult<u32> {
        let latest_checkpoint = match self
            .sui_client.read_api().get_latest_checkpoint_sequence_number().await {
                Ok(checkpoint) => checkpoint,
                Err(e) => return Err(ChainCommunicationError::from_other(e).into()),
            };

        Ok(latest_checkpoint as u32)
    }
}