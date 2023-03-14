use awc::error::{JsonPayloadError, SendRequestError};

/// Error type
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Invalid URL
    #[error("invalid url")]
    InvalidUrl,
    /// RpcError wrapping awc::error::SendRequestError
    #[error("rpc error {0}")]
    RpcError(#[from] SendRequestError),
    /// JsonError wrapping awc::error::JsonPayloadError
    #[error("rpc error {0}")]
    JsonError(#[from] JsonPayloadError),
    /// API call failed
    #[error("api error {0}")]
    ApiError(String),
    /// Returned when tx construction fails (code, message)
    #[error("tx construction failed {0}")]
    TxConstructionFailed(String, String),
    /// Transaction failed on chain
    #[error("tx failed {0}")]
    TxFailed(String),
    /// Transaction timeout
    #[error("transaction timeout")]
    TxTimeout,
    /// Returned when contract query fails
    #[error("contract query failed {0} {1}")]
    ContractQueryFailed(String, String),
    /// Returned when RPC returns invalid or unknown response
    #[error("rpc returned unknown response type")]
    UnknownResponse(String),
    /// Invalid index passed
    #[error("invalid index")]
    InvalidIndex,
    /// Contract does not exists
    #[error("contract not found")]
    ContractNotFound,
    /// Account does not exists
    #[error("account not found")]
    AccountNotFound,
}
