use hyperlane_core::{ChainCommunicationError, ChainResult, ContractLocator, HyperlaneDomain};
use sui_sdk::types::base_types::SuiAddress;

use crate::{ConnectionConf, SuiRpcClient};

/// A reference to a Mailbox contract on some Sui chain
pub struct SuiMailbox {
    pub(crate) domain: HyperlaneDomain,
    payer: Option<SuiAddress>,
    pub(crate) sui_client: SuiRpcClient,
    pub(crate) packages_address: SuiAddress,
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
            domain: locator.domain.clone(),
            sui_client,
            packages_address: package_address,
            payer,
        })
    }

    async fn fetch_module_name(&self, package_address: &SuiAddress) -> ChainResult<Vec<u8>> {
        
        let view_response = utils::send_owned_objects_request().await?;
    }
}
