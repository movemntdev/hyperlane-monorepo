use std::str::FromStr;

use async_trait::async_trait;
use base64::write;
use hyperlane_core::{
    ChainCommunicationError, ChainResult, ContractLocator, HyperlaneChain, HyperlaneContract,
    HyperlaneDomain, HyperlaneProvider, Mailbox, H256,
};
use sui_sdk::{
    json::{MoveTypeLayout, SuiJsonValue},
    types::{base_types::SuiAddress, transaction::CallArg},
};
use tracing::instrument;
use url::Url;

use crate::{
    move_view_call, send_owned_objects_request, AddressFormatter, ConnectionConf, SuiHpProvider,
    SuiRpcClient, TryIntoPrimitive,
};

/// A reference to a Mailbox contract on some Sui chain
pub struct SuiMailbox {
    pub(crate) domain: HyperlaneDomain,
    payer: Option<SuiAddress>,
    pub(crate) sui_client: SuiRpcClient,
    pub(crate) packages_address: SuiAddress,
    rest_url: Url,
}

impl SuiMailbox {
    /// Create a new Sui Mailbox
    pub fn new(
        conf: &ConnectionConf,
        locator: ContractLocator,
        payer: Option<SuiAddress>,
    ) -> ChainResult<Self> {
        let package_address = SuiAddress::from_bytes(<[u8; 32]>::from(locator.address)).unwrap();
        let sui_client = tokio::runtime::Runtime::new()
            .expect("Failed to create runtime")
            .block_on(async { SuiRpcClient::new(conf.url.to_string()).await })
            .expect("Failed to create SuiRpcClient");
        Ok(Self {
            domain: *locator.domain,
            rest_url: conf.url,
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
        let (bytes, type_tag) = view_response[0].return_values[0];
        let delivered_json = SuiJsonValue::from_bcs_bytes(Some(&MoveTypeLayout::Bool), &bytes).unwrap();
        Ok(delivered_json.try_into_bool().expect("Failed to convert to bool"))
    }

    #[instrument(err, ret, skip(self))]
    async fn default_ism(&self) ->ChainResult<H256> {
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

        let (bytes, type_tag) = view_response[0].return_values[0];
        let ism_json = SuiJsonValue::from_bcs_bytes(Some(&MoveTypeLayout::Address), &bytes).unwrap();
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

        let (bytes, type_tag) = view_response[0].return_values[0];
        let ism_json = SuiJsonValue::from_bcs_bytes(Some(&MoveTypeLayout::Address), &bytes).unwrap();
        Ok(ism_json.try_into_h256().expect("Failed to convert to H256"))
    }
}
