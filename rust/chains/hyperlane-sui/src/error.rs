use hyperlane_core::ChainCommunicationError;
use sui_sdk::error;

/// Errors from the crates specific to the hyperlane-cosmos
/// implementation.
/// This error can then be converted into the broader error type
/// in hyperlane-core using the `From` trait impl
#[derive(Debug, thiserror::Error)]
pub enum HyperlaneSuiError {
    ///Sui error report
    #[error("{0}")]
    SuiError(#[from] sui_sdk::error::Error),
    
}

impl From<HyperlaneSuiError> for ChainCommunicationError {
    fn from(value: HyperlaneSuiError) -> Self {
        ChainCommunicationError::from_other(value)
    }
}