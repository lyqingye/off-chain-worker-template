//! This module defines the various errors that be raised in the relayer.

use anomaly::{BoxError, Context};
use thiserror::Error;
use tendermint::chain::Id as ChainId;

/// An error that can be raised by the relayer.
pub type Error = anomaly::Error<Kind>;

/// Various kinds of errors that can be raiser by the relayer.
#[derive(Clone, Debug, Error)]
pub enum Kind {
    /// Config I/O error
    #[error("config I/O error")]
    ConfigIo,

    /// I/O error
    #[error("I/O error")]
    Io,

    /// Invalid configuration
    #[error("invalid configuration")]
    Config,

    /// RPC error (typically raised by the RPC client or the RPC requester)
    #[error("RPC error to endpoint {0}")]
    Rpc(tendermint_rpc::Url),

    /// Websocket error (typically raised by the Websocket client)
    #[error("Websocket error to endpoint {0}")]
    Websocket(tendermint_rpc::Url),

    /// Event monitor error
    #[error("event monitor error: {0}")]
    EventMonitor(crate::subscribe::monitor::Error),

    /// GRPC error (typically raised by the GRPC client or the GRPC requester)
    #[error("GRPC error")]
    Grpc,

    /// Event error (raised by the event monitor)
    #[error("Bad Notification")]
    Event,

    /// Invalid height
    #[error("Invalid height")]
    InvalidHeight,

    /// Did not find tx confirmation
    #[error("did not find tx confirmation {0}")]
    TxNoConfirmation(String),

    /// Gas estimate from simulated Tx exceeds the maximum configured
    #[error("{chain_id} gas estimate {estimated_gas} from simulated Tx exceeds the maximum configured {max_gas}")]
    TxSimulateGasEstimateExceeded {
        chain_id: ChainId,
        estimated_gas: u64,
        max_gas: u64,
    },

    /// A message transaction failure
    #[error("Message transaction failure: {0}")]
    MessageTransaction(String),

    /// Failed query
    #[error("Query error occurred (failed to query for {0})")]
    Query(String),

    /// Keybase related error
    #[error("Keybase error")]
    KeyBase,

    /// Invalid chain identifier
    #[error("invalid chain identifier format: {0}")]
    ChainIdentifier(String),

    #[error("invalid key address: {0}")]
    InvalidKeyAddress(String),

    #[error("bech32 encoding failed")]
    Bech32Encoding(#[from] bech32::Error),

    #[error("health check failed for endpoint {endpoint} on the Json RPC interface of chain {chain_id}:{address}; caused by: {cause}")]
    HealthCheckJsonRpc {
        chain_id: ChainId,
        address: String,
        endpoint: String,
        cause: tendermint_rpc::error::Error,
    },

    #[error("health check failed for service {endpoint} on the gRPC interface of chain {chain_id}:{address}; caused by: {cause}")]
    HealthCheckGrpc {
        chain_id: ChainId,
        address: String,
        endpoint: String,
        cause: String,
    },

    #[error("Hermes health check failed while verifying the application compatibility for chain {chain_id}:{address}; caused by: {cause}")]
    SdkModuleVersion {
        chain_id: ChainId,
        address: String,
        cause: String,
    },
}

impl Kind {
    /// Add a given source error as context for this error kind
    ///
    /// This is typically use with `map_err` as follows:
    ///
    /// ```ignore
    /// let x = self.something.do_stuff()
    ///     .map_err(|e| error::Kind::Config.context(e))?;
    /// ```
    pub fn context(self, source: impl Into<BoxError>) -> Context<Self> {
        Context::new(self, Some(source.into()))
    }
}
