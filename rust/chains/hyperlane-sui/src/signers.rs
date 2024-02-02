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

/// A signer is a key pair that can sign transactions.
/// For now we only support ED25519.
#[derive(Debug)]
pub struct Signer {
    /// Sui address
    pub address: SuiAddress,
    /// SuiKeyPair
    pub key_pair: SuiKeyPair,
}

impl Signer {
    /// Derive a signer from an ED25519 private key.
    pub fn new(private_key: &str) -> ChainResult<Self> {
        //TODO: remove unrwaps & expects
        let bytes = Base64::decode(private_key).expect("Invalid base64");
        let k = SuiKeyPair::Ed25519(Ed25519KeyPair::from_bytes(&bytes).unwrap());
        Ok(Self {
            address: SuiAddress::from(&k.public()),
            key_pair: k,
        })
    }
}
