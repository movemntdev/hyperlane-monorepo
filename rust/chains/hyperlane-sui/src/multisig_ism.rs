use async_trait::async_trait;
use hyperlane_core::{
    ChainResult, ContractLocator, HyperlaneChain, HyperlaneContract, HyperlaneDomain,
    HyperlaneMessage, HyperlaneProvider, MultisigIsm, H256,
};
use move_core_types::annotated_value::MoveTypeLayout;
use solana_sdk::signature::Keypair;
use sui_sdk::{
    json::SuiJsonValue,
    types::{base_types::SuiAddress, transaction::CallArg},
};

use crate::{
    move_view_call, AddressFormatter, ConnectionConf, Signer, SuiHpProvider, SuiRpcClient, TryIntoPrimitive, Validators
};

///A reference to a MultsigIsm module on a Sui Chain.
#[derive(Debug)]
pub struct SuiMultisigISM {
    signer: Option<Signer>, //field never read
    domain: HyperlaneDomain,
    sui_client: SuiRpcClient,
    package_address: SuiAddress,
    rest_url: String,
}

impl SuiMultisigISM {
    ///Create a new Sui Multisig ISM.
    pub fn new(conf: &ConnectionConf, locator: ContractLocator, signer: Option<Signer>) -> Self {
        let package_address = SuiAddress::from_bytes(<[u8; 32]>::from(locator.address)).unwrap();
        let sui_client = tokio::runtime::Runtime::new()
            .expect("Failed to create runtime")
            .block_on(async { SuiRpcClient::new(conf.url.to_string()).await })
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

impl HyperlaneContract for SuiMultisigISM {
    fn address(&self) -> H256 {
        self.package_address.to_bytes().into()
    }
}

impl HyperlaneChain for SuiMultisigISM {
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
impl MultisigIsm for SuiMultisigISM {
    /// Returns the validator and threshold needed to verify a message
    async fn validators_and_threshold(
        &self,
        message: &HyperlaneMessage,
    ) -> ChainResult<(Vec<H256>, u8)> {
        let view_response = move_view_call(
            &self.sui_client,
            &self.package_address,
            self.package_address,
            "multisig_ism".to_string(),
            "validators_and_threshold".to_string(),
            vec![],
            vec![CallArg::Pure(
                bcs::to_bytes(&message.origin).expect("Failed to serialize origin"),
            )],
        )
        .await?;
        let (bytes, type_tag) = &view_response[0].return_values[0];

        // There is no MoveStructLayout for tuple types, which this move fn returns,
        // so trying to parse it with None, seems risky.
        let validators_json =
            SuiJsonValue::from_bcs_bytes(None, bytes).expect("Failed to deserialize validators");
        let validators = validators_json
            .try_into_validators()
            .expect("Failed to convert to Vec<H256>");
        Ok(validators)
    }
}
