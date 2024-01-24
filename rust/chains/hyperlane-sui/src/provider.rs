use std::str::FromStr;
use hyperlane_core::{
    BlockInfo, ChainCommunicationError, ChainResult, HyperlaneChain, HyperlaneDomain, HyperlaneProvider, TxnInfo, H256, U256
};
use anyhow::Error;
use async_trait::async_trait;
use sui_sdk::{types::base_types::SuiAddress, SuiClient};
use crate::SuiRpcClient;

/// A wrapper around a Sui provider to get generic blockchain information.
#[derive(Debug)]
pub struct SuiHpProvider {
    domain: HyperlaneDomain,
    sui_client: SuiRpcClient,
    rest_url: String,
}

impl SuiHpProvider {
    /// Create a new Sui provider.
    pub async fn new(domain: HyperlaneDomain, rest_url: String) -> Result<Self, Error>{
        let sui_client = SuiRpcClient::new(rest_url.clone()).await?;
            Ok(Self {
                domain,
                sui_client,
                rest_url
            })
    }
}

impl HyperlaneChain for SuiHpProvider {
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

#[async_trait]
impl HyperlaneProvider for SuiHpProvider {
    async fn get_block_by_hash(&self, _has: &H256) -> ChainResult<BlockInfo> {
        todo!() // Cannot get block as Sui is DAG based. have to get checkpoint instead.
    }

    async fn get_txn_by_hash(&self, hash: &H256) -> ChainResult<TxnInfo> {
        todo!() // Cannot get by hash but have to get by Transaction Digest intead. 
    }

    async fn is_contract(&self, _address: &H256) -> ChainResult<bool> {
        // Sui account can be both normal account & contract account
        Ok(true)
    }

    async fn get_balance(&self, address: String) -> ChainResult<U256> {
        let coin_type = "0x2::sui::SUI".to_string();
        let balance = match self
            .sui_client
            .coin_read_api()
            .get_balance(SuiAddress::from_str(&address).unwrap(), Some(coin_type)).await {
                Ok(balance) => balance,
                Err(e) => return Err(ChainCommunicationError::from_other(e).into()),
            };
        Ok(balance.total_balance.into())
    }
}