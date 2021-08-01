use std::sync::Arc;

use prost_types::Any;
use tokio::runtime::Runtime as TokioRuntime;

use crate::events::Event;
use crate::error::Error;
use crate::subscribe::monitor::{TxMonitorCmd, EventReceiver};
use crate::keyring::{KeyEntry, KeyRing};
use tendermint::chain::Id as ChainId;
use tendermint::abci::transaction::Hash;
use crate::config::ChainConfig;


pub mod cosmos;
pub mod runtime;
pub mod handle;

/// Used for queries and not yet standardized in channel's query.proto
#[derive(Clone, Debug)]
pub enum QueryTxRequest {
    Transaction(QueryTxHash),
}

#[derive(Clone, Debug)]
pub struct QueryTxHash(pub Hash);

/// Defines a blockchain as understood by the relayer
pub trait Chain: Sized {
    /// Constructs the chain
    fn bootstrap(config: ChainConfig,rt: Arc<TokioRuntime>) -> Result<Self, Error>;

    /// Initializes and returns the event monitor (if any) associated with this chain.
    fn init_event_monitor(
        &self,
        rt: Arc<TokioRuntime>,
    ) -> Result<(EventReceiver, TxMonitorCmd), Error>;

    fn shutdown(self) -> Result<(), Error>;

    /// Returns the chain's identifier
    fn id(&self) -> &ChainId;

    /// Returns the chain's keybase
    fn keybase(&self) -> &KeyRing;

    /// Returns the chain's keybase, mutably
    fn keybase_mut(&mut self) -> &mut KeyRing;

    /// Get the account for the signer
    fn get_signer(&mut self) -> Result<String, Error>;

    /// Sends one or more transactions with `msgs` to chain.
    fn send_msgs(&mut self, proto_msgs: Vec<Any>) -> Result<Vec<Event>, Error>;

    // key for sign message
    fn get_key(&mut self) -> Result<KeyEntry, Error>;

    // query event from tx response
    fn query_events_from_txs(&self, request: QueryTxRequest) -> Result<Vec<Event>, Error>;
}
