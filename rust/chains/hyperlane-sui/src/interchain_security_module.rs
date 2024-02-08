use async_trait::async_trait;
use hyperlane_core::{
    ChainCommunicationError, ChainResult, ContractLocator, HyperlaneChain, HyperlaneContract,
    HyperlaneDomain, HyperlaneMessage, HyperlaneProvider, InterchainSecurityModule, ModuleType,
    H256, U256,
};
use move_core_types::annotated_value::MoveTypeLayout;
use num_traits::cast::FromPrimitive;
use solana_sdk::signature::Keypair;
use sui_sdk::{
    json::SuiJsonValue,
    types::base_types::{ObjectID, SuiAddress},
};

use crate::{
    move_view_call, AddressFormatter, ConnectionConf, Signer, SuiHpProvider, SuiRpcClient,
    TryIntoPrimitive,
};

#[derive(Debug)]
pub struct SuiInterchainSecurityModule {
    sui_client: SuiRpcClient,
    package: ObjectID,
    signer: Option<Signer>,
    domain: HyperlaneDomain,
    rest_url: String,
}

impl SuiInterchainSecurityModule {
    /// Create a new Sui Interchain Security Module.
    pub fn new(conf: &ConnectionConf, locator: ContractLocator, signer: Option<Signer>) -> Self {
        let package = locator
            .modules
            .as_ref()
            .expect("ISM module not found")
            .get("hp_isms")
            .expect("ISM module not found")
            .clone();

        let sui_client = tokio::runtime::Runtime::new()
            .expect("Failed to create runtime")
            .block_on(async { SuiRpcClient::new().await })
            .expect("Failed to create SuiRpcClient");
        Self {
            domain: locator.domain.clone(),
            rest_url: conf.url.to_string(),
            sui_client,
            package,
            signer,
        }
    }
}

impl HyperlaneContract for SuiInterchainSecurityModule {
    fn address(&self) -> hyperlane_core::H256 {
        self.package.into_bytes().into()
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
        let signer = self.signer.as_ref().expect("Signer not found");
        let view_response = move_view_call(
            &self.sui_client,
            &signer.address,
            self.package.into(),
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

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use hyperlane_core::utils::hex_or_base58_to_h256;
    use sui_sdk::types::base_types::ObjectID;
    use url::Url;

    use super::*;

    const OPERATOR_ADDRESS: &str =
        "0x7d0f597d041f441d3821c1e2562226898b96a2b0e67e178eacf43c0f2f5188f2";
    const ISMS_OBJECT_ID: &str =
        "0x41f95774097a22932a5016442d3c81f4a73ce4e4e23dfd245986e64862bfbe5a";
    const ISMS_MODULE_NAME: &str = "hp_isms";

    fn init_interchain_security_module() -> SuiInterchainSecurityModule {
        let addr = hex_or_base58_to_h256(OPERATOR_ADDRESS).unwrap();
        let obj_hex = hex_or_base58_to_h256(ISMS_OBJECT_ID).unwrap();
        let object_id =
            ObjectID::try_from(SuiAddress::from_bytes(<[u8; 32]>::from(obj_hex)).unwrap()).unwrap();

        let conf = ConnectionConf {
            url: Url::parse("http://localhost:8080").unwrap(),
        };
        let locator = ContractLocator {
            address: addr,
            domain: &HyperlaneDomain::Known(hyperlane_core::KnownHyperlaneDomain::Fuji),
            modules: Some(HashMap::from_iter(vec![(
                ISMS_MODULE_NAME.to_string(),
                object_id,
            )])),
        };
        SuiInterchainSecurityModule::new(&conf, locator, None)
    }

    #[test]
    fn test_should_create_new_interchain_security_module() {
        let isms = init_interchain_security_module();
        let obj_hex = hex_or_base58_to_h256(ISMS_OBJECT_ID).unwrap();
        assert_eq!(isms.address(), obj_hex)
    }

    #[test]
    fn test_should_return_domain_for_interchain_security_module() {
        let isms = init_interchain_security_module();
        assert_eq!(
            isms.domain(),
            &HyperlaneDomain::Known(hyperlane_core::KnownHyperlaneDomain::Fuji)
        )
    }

    #[test]
    fn test_should_return_provider_for_gas_paymaster() {
        let isms = init_interchain_security_module();
        let provider = isms.provider();
        assert_eq!(
            provider.domain(),
            &HyperlaneDomain::Known(hyperlane_core::KnownHyperlaneDomain::Fuji)
        )
    }

    // NOTE: currently this test fails because init_interchain_security_module() panics
    // calls new, which creates a new tokio runtime and blocks the async Sui client call
    // it's not possible to create a new tokio runtime in a tokio::test. 
    // One way to resolve this is with lazy_static, for new() but I didn't want 
    // to massively alter the implementation yet, if the blocking call is still a problem
    // beyond this test, we can look into it further.
    #[tokio::test]
    async fn test_should_return_module_type() {
        let isms = init_interchain_security_module();
        let module_type_future = tokio::spawn(async move { isms.module_type().await.unwrap() });

        // Wait for the future to complete and get the result.
        let module_type = module_type_future.await.unwrap();
        assert_eq!(module_type, ModuleType::MerkleRootMultisig);
    }
}
