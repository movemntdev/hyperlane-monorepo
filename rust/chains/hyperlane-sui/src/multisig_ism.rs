use async_trait::async_trait;
use hyperlane_core::{
    ChainCommunicationError, ChainResult, ContractLocator, HyperlaneChain, HyperlaneContract, HyperlaneDomain, HyperlaneMessage, HyperlaneProvider, MultisigIsm, H256
};
use sui_sdk::{
    json::SuiJsonValue,
    types::{base_types::{ObjectID, SuiAddress}, transaction::CallArg},
};

use crate::{
    move_view_call, ConnectionConf, Signer, SuiHpProvider, SuiRpcClient, TryIntoPrimitive, Validators
};

///A reference to a MultsigIsm module on a Sui Chain.
#[derive(Debug)]
pub struct SuiMultisigISM {
    signer: Option<Signer>, 
    domain: HyperlaneDomain,
    sui_client: SuiRpcClient,
    package: ObjectID,
    url: String,
}

impl SuiMultisigISM {
    ///Create a new Sui Multisig ISM.
    pub fn new(conf: &ConnectionConf, locator: ContractLocator, signer: Option<Signer>) -> Self {
        let package = locator
            .modules
            .as_ref()
            .expect("ISM module not found")
            .get("multisig_ism")
            .expect("ISM module not found")
            .clone();
        let sui_client = tokio::runtime::Runtime::new()
            .expect("Failed to create runtime")
            .block_on(async { SuiRpcClient::new().await })
            .expect("Failed to create SuiRpcClient");
        Self {
            domain: locator.domain.clone(),
            url: conf.url.to_string(),
            sui_client,
            package,
            signer,
        }
    }
}

impl HyperlaneContract for SuiMultisigISM {
    fn address(&self) -> H256 {
        self.package.into_bytes().into()
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
                SuiHpProvider::new(self.domain.clone(), self.url.clone()).await
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
        let signer = self
            .signer
            .as_ref()
            .ok_or_else(|| ChainCommunicationError::from_contract_error_str("No signer provided"))?;
        let view_response = move_view_call(
            &self.sui_client,
            &signer.address,
            self.package.into(),
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
