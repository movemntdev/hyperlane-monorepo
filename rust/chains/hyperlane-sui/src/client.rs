use sui_sdk::{SuiClientBuilder, SuiClient};

/// Sui RPC client
pub struct SuiRpcClient(SuiClient);
impl SuiRpcClient {
    /// Create a new aptos rpc client from node url
    pub async fn new() -> Result<Self, anyhow::Error> {
      // TODO: feature flag for testnet/mainnet
      let client = SuiClientBuilder::default().build_testnet().await?;
      Ok(Self(client))
    }
}

impl std::ops::Deref for SuiRpcClient {
    type Target = SuiClient;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::fmt::Debug for SuiRpcClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SuiRpcClient { ... }")
    }
}

mod tests {
    use crate::SuiRpcClient;

    #[tokio::test]
    async fn test_creates_new_client() {
        let client = SuiRpcClient::new().await.unwrap();
    }
}

