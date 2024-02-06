use async_trait::async_trait;
use hyperlane_core::{
    ChainCommunicationError, ChainResult, ContractLocator, HyperlaneChain, HyperlaneContract,
    HyperlaneDomain, HyperlaneMessage, HyperlaneProvider, InterchainSecurityModule, ModuleType,
    H256, U256,
};
use move_core_types::annotated_value::MoveTypeLayout;
use num_traits::cast::FromPrimitive;
use solana_sdk::signature::Keypair;
use sui_sdk::{json::SuiJsonValue, types::base_types::SuiAddress};

use crate::{
    move_view_call, AddressFormatter, ConnectionConf, Signer, SuiHpProvider, SuiRpcClient, TryIntoPrimitive
};

#[derive(Debug)]
pub struct SuiInterchainSecurityModule {
    sui_client: SuiRpcClient,
    package_address: SuiAddress,
    signer: Option<Signer>,
    domain: HyperlaneDomain,
    rest_url: String,
}

impl SuiInterchainSecurityModule {
    /// Create a new Sui Interchain Security Module.
    pub fn new(conf: &ConnectionConf, locator: ContractLocator, signer: Option<Signer>) -> Self {
        let package_address = SuiAddress::from_bytes(<[u8; 32]>::from(locator.address)).unwrap();
        let sui_client = tokio::runtime::Runtime::new()
            .expect("Failed to create runtime")
            .block_on(async { SuiRpcClient::new().await })
            .expect("Failed to create SuiRpcClient");
        Self {
            domain: locator.domain.clone(),
            rest_url: conf.url.to_string(),
            sui_client,
            package_address,
            signer,
        }
    }
}

impl HyperlaneContract for SuiInterchainSecurityModule {
    fn address(&self) -> hyperlane_core::H256 {
        self.package_address.to_bytes().into()
    }
}

impl HyperlaneChain for SuiInterchainSecurityModule {
    fn domain(&self) -> &HyperlaneDomain {
        &self.domain
    }

    fn provider(&self) -> Box<dyn HyperlaneProvider> {
        let sui_provider = tokio::runtime::Runtime::new()
            .expect("Failed to create runtime")
            .block_on(async {
                SuiHpProvider::new(self.domain.clone(), self.rest_url.clone()).await
            });
        Box::new(sui_provider)
    }
}

#[async_trait]
impl InterchainSecurityModule for SuiInterchainSecurityModule {
    async fn module_type(&self) -> ChainResult<ModuleType> {
        let view_response = move_view_call(
            &self.sui_client,
            &self.package_address,
            self.package_address,
            "multisig_ism".to_string(),
            "get_module_type".to_string(),
            vec![],
            vec![],
        )
        .await?;
        let (bytes, type_tag) = &view_response[0].return_values[0];
        let module = SuiJsonValue::from_bcs_bytes(Some(&MoveTypeLayout::U64), bytes)
            .expect("Failed to deserialize module type")
            .try_into_u64()
            .expect("Failed to convert to u64");

        if let Some(module_type) = ModuleType::from_u64(module) {
            Ok(module_type)
        } else {
            Err(ChainCommunicationError::from_contract_error_str(
                "Invalid module type",
            ))
        }
    }

    async fn dry_run_verify(
        &self,
        _message: &HyperlaneMessage,
        _metadata: &[u8],
    ) -> ChainResult<Option<U256>> {
        // TODO: Implement this once we have aggregation ISM support in Sui
        Ok(Some(U256::zero()))
    }
}
