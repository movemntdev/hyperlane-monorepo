use fastcrypto::{
    ed25519::Ed25519KeyPair,
    encoding::{Base64, Encoding},
    traits::ToFromBytes,
};
use hyperlane_core::{ChainCommunicationError, ChainResult};
use sui_sdk::types::{
    base_types::SuiAddress,
    crypto::{PublicKey, SuiKeyPair},
};

type PrivateKey = PublicKey;

#[derive(Clone, Debug)]
pub struct Signer {
    /// public key
    pub public_key: PublicKey,
    pub address: String,
    private_key: PrivateKey,
}

impl Signer {
    /// Derive a signer from an ED25519 private key.
    pub fn new(private_key: &str) -> ChainResult<Self> {
        // TODO remove unwraps.
        let k = SuiKeyPair::Ed25519(
            Ed25519KeyPair::from_bytes(&Base64::decode(private_key).unwrap()).unwrap(),
        );
        let priv_k = k.public();

        Ok(Self {
            public_key: PublicKey::Ed25519(pk.to_vec()),
            private_key: priv_k,
            address: SuiAddress::from(&priv_k),
        })
    }
}
