use std::{ops::RangeInclusive, str::FromStr};

use crate::{
    get_filtered_events, ConnectionConf, EventSourceLocator, FilterBuilder, GasPaymentEventData,
    SuiHpProvider, SuiRpcClient,
};
use ::sui_sdk::types::base_types::SuiAddress;
use async_trait::async_trait;
use hex;
use hyperlane_core::{
    to_hex, ChainCommunicationError, ChainResult, ContractLocator, HyperlaneChain,
    HyperlaneContract, HyperlaneDomain, HyperlaneProvider, Indexer, InterchainGasPaymaster,
    InterchainGasPayment, LogMeta, SequenceIndexer, H256,
};
use move_core_types::identifier::Identifier;
use sui_sdk::types::{base_types::ObjectID, digests::TransactionDigest};
use tracing::{info, instrument};

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
    package: ObjectID,
    url: String,
}

impl SuiInterchainGasPaymaster {
    /// Create a new Sui IGP.
    pub fn new(conf: &ConnectionConf, locator: &ContractLocator) -> ChainResult<Self> {
        if let Some(package_id) = locator
            .modules
            .clone()
            .expect("No modules found for Sui IGP contract")
            .get("hp_igps")
        {
            return Ok(Self {
                domain: locator.domain.clone(),
                package: *package_id,
                url: conf.url.to_string(),
            });
        } else {
            Err(ChainCommunicationError::from_other_str(
                "No module found for Sui IGP contract",
            ))
        }
    }
}

impl HyperlaneContract for SuiInterchainGasPaymaster {
    /// Return the address of the Sui IGP contract. 
    /// In Sui this is refered to as the ObjectID. 
    fn address(&self) -> H256 {
        self.package.into_bytes().into()
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
                SuiHpProvider::new(self.domain.clone(), self.url.clone()).await
            });
        Box::new(sui_provider)
    }
}

impl InterchainGasPaymaster for SuiInterchainGasPaymaster {}

/// Struct that retrieves event data for a Sui IGP contract.
#[derive(Debug)]
pub struct SuiInterchainGasPaymasterIndexer {
    sui_client: SuiRpcClient,
    package: ObjectID,
    ident: Identifier,
}

impl FilterBuilder for SuiInterchainGasPaymasterIndexer {}

impl EventSourceLocator for SuiInterchainGasPaymasterIndexer {
    fn package(&self) -> ObjectID {
        self.package
    }

    fn identifier(&self) -> Identifier {
        self.ident.clone()
    }
}

impl SuiInterchainGasPaymasterIndexer {
    /// Create a new Sui IGP indexer.
    pub fn new(conf: &ConnectionConf, locator: ContractLocator) -> ChainResult<Self> {
        let ident = "hp_igps";
        let sui_client = tokio::runtime::Runtime::new()
            .expect("Failed to create runtime")
            .block_on(async { SuiRpcClient::new().await })
            .expect("Failed to create SuiRpcClient");
        if let Some(package_id) = locator
            .modules
            .clone()
            .expect("No modules found for Sui IGP contract")
            .get(ident)
        {
            return Ok(Self {
                sui_client,
                package: *package_id,
                ident: Identifier::new(ident).expect("Failed to create Identifier"),
            });
        } else {
            Err(ChainCommunicationError::from_other_str(
                "No module found for Sui IGP contract",
            ))
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
        get_filtered_events::<InterchainGasPayment, GasPaymentEventData>(
            &self.sui_client,
            self.package(),
            self.identifier(),
            self.build_filter("GasPaymentEvent", range),
        )
        .await
    }

    /// Sui is a DAG-based blockchain and uses checkpoints for node
    /// synchronization and global transaction ordering. So this method when
    /// implemented for `SuiInterchainGasPaymasterIndexer` will return the
    /// latest checkpoint sequence number.
    #[instrument(level = "debug", err, ret, skip(self))]
    async fn get_finalized_block_number(&self) -> ChainResult<u32> {
        let latest_checkpoint = match self
            .sui_client
            .read_api()
            .get_latest_checkpoint_sequence_number()
            .await
        {
            Ok(checkpoint) => checkpoint,
            Err(e) => return Err(ChainCommunicationError::from_other(e).into()),
        };

        Ok(latest_checkpoint as u32)
    }
}

#[async_trait]
impl SequenceIndexer<InterchainGasPayment> for SuiInterchainGasPaymasterIndexer {
    // Same as aptos, this is not fully implemented yet.
    async fn sequence_and_tip(&self) -> ChainResult<(Option<u32>, u32)> {
        let tip = self.get_finalized_block_number().await?;
        Ok((None, tip))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{collections::HashMap, ops::RangeInclusive};

    use hyperlane_core::{
        utils::hex_or_base58_to_h256, ContractLocator, HyperlaneContract, HyperlaneDomain, Indexer,
        KnownHyperlaneDomain,
    };
    use move_core_types::identifier::Identifier;
    use sui_sdk::types::base_types::{ObjectID, SuiAddress};
    use url::Url;

    const OPERATOR_ADDRESS: &str =
        "0x7d0f597d041f441d3821c1e2562226898b96a2b0e67e178eacf43c0f2f5188f2";
    const IGPS_OBJECT_ID: &str =
        "0x41f95774097a22932a5016442d3c81f4a73ce4e4e23dfd245986e64862bfbe5a";
    const IGPS_MODULE_NAME: &str = "hp_igps";

    fn init_gas_paymaster() -> SuiInterchainGasPaymaster {
        let addr = hex_or_base58_to_h256(OPERATOR_ADDRESS).unwrap();
        let obj_hex = hex_or_base58_to_h256(IGPS_OBJECT_ID).unwrap();
        let object_id =
            ObjectID::try_from(SuiAddress::from_bytes(<[u8; 32]>::from(obj_hex)).unwrap()).unwrap();

        // empty Conf as Sui connection doesn't need it
        let conf = crate::ConnectionConf {
            // give URL some value, Sui does nothing with this.
            url: Url::parse("http://localhost:8079").unwrap(),
        };

        let locator = ContractLocator {
            address: addr,
            domain: &HyperlaneDomain::Known(KnownHyperlaneDomain::Fuji),
            modules: Some(HashMap::from_iter(vec![(
                IGPS_MODULE_NAME.to_string(),
                object_id,
            )])),
        };
        SuiInterchainGasPaymaster::new(&conf, &locator).unwrap()
    }

    #[test]
    fn test_should_create_new_gas_paymaster() {
        let paymaster = init_gas_paymaster();
        let addr = hex_or_base58_to_h256(IGPS_OBJECT_ID).unwrap();
        assert_eq!(
            paymaster.package,
            ObjectID::try_from(SuiAddress::from_bytes(addr).unwrap()).unwrap()
        );
    }

    #[test]
    fn test_should_return_address_as_h256_for_gas_paymaster() {
        let paymaster = init_gas_paymaster();
        let addr = hex_or_base58_to_h256(IGPS_OBJECT_ID).unwrap();
        assert_eq!(paymaster.address(), addr);
    }

    #[test]
    fn test_should_return_domain_for_gas_paymaster() {
        let paymaster = init_gas_paymaster();
        assert_eq!(
            paymaster.domain(),
            &HyperlaneDomain::Known(KnownHyperlaneDomain::Fuji)
        );
    }

    #[test]
    fn test_should_return_provider_for_gas_paymaster() {
        let paymaster = init_gas_paymaster();
        let provider = paymaster.provider();
        assert_eq!(
            provider.domain(),
            &HyperlaneDomain::Known(KnownHyperlaneDomain::Fuji)
        );
    }

    #[test]
    fn test_should_create_new_igp_indexer() {
        let addr = hex_or_base58_to_h256(OPERATOR_ADDRESS).unwrap();
        let obj_hex = hex_or_base58_to_h256(IGPS_OBJECT_ID).unwrap();
        let object_id =
            ObjectID::try_from(SuiAddress::from_bytes(<[u8; 32]>::from(obj_hex)).unwrap()).unwrap();

        // empty Conf as Sui connection doesn't need it
        let conf = crate::ConnectionConf {
            // give URL some value, Sui does nothing with this.
            url: Url::parse("http://localhost:8080").unwrap(),
        };

        let locator = ContractLocator {
            address: addr,
            domain: &HyperlaneDomain::Known(KnownHyperlaneDomain::Fuji),
            modules: Some(HashMap::from_iter(vec![(
                IGPS_MODULE_NAME.to_string(),
                object_id.clone(),
            )])),
        };
        let indexer = SuiInterchainGasPaymasterIndexer::new(&conf, locator).unwrap();
        assert_eq!(indexer.package, ObjectID::try_from(object_id).unwrap());
        assert_eq!(indexer.ident, Identifier::new(IGPS_MODULE_NAME).unwrap());
    }

    #[test]
    fn test_indexer_should_fetch_logs_from_gas_payment_event() {
        let addr = hex_or_base58_to_h256(OPERATOR_ADDRESS).unwrap();
        let obj_hex = hex_or_base58_to_h256(IGPS_OBJECT_ID).unwrap();
        let object_id =
            ObjectID::try_from(SuiAddress::from_bytes(<[u8; 32]>::from(obj_hex)).unwrap()).unwrap();

        // empty Conf as Sui connection doesn't need it
        let conf = crate::ConnectionConf {
            // give URL some value, Sui does nothing with this.
            url: Url::parse("http://localhost:8080").unwrap(),
        };

        let locator = ContractLocator {
            address: addr,
            domain: &HyperlaneDomain::Known(KnownHyperlaneDomain::Fuji),
            modules: Some(HashMap::from_iter(vec![(
                IGPS_MODULE_NAME.to_string(),
                object_id,
            )])),
        };
        let indexer = crate::SuiInterchainGasPaymasterIndexer::new(&conf, locator).unwrap();
        let logs = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(indexer.fetch_logs(RangeInclusive::new(0, 10)))
            .unwrap();

        assert_eq!(logs.len(), 0);
    }
}
