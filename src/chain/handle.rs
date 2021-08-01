use crossbeam_channel as channel;
use dyn_clone::DynClone;
use std::{fmt::Debug, sync::Arc};

use crate::{
    chain::QueryTxRequest,
    error::Error,
    events::Event,
    keyring::KeyEntry,
    subscribe::monitor::{EventBatch, Result as MonitorResult},
};
use serde::{Serialize, Serializer};
use tendermint::chain::Id as ChainId;

pub type Subscription = channel::Receiver<Arc<MonitorResult<EventBatch>>>;
pub type ReplyTo<T> = channel::Sender<Result<T, Error>>;
pub type Reply<T> = channel::Receiver<Result<T, Error>>;

pub fn reply_channel<T>() -> (ReplyTo<T>, Reply<T>) {
    channel::bounded(1)
}

pub trait ChainHandle: DynClone + Send + Sync + Debug {
    /// Get the [`ChainId`] of this chain.
    fn id(&self) -> ChainId;

    /// Shutdown the chain runtime.
    fn shutdown(&self) -> Result<(), Error>;

    /// Subscribe to the events emitted by the chain.
    fn subscribe(&self) -> Result<Subscription, Error>;

    /// Send the given `msgs` to the chain, packaged as one or more transactions,
    /// and return the list of events emitted by the chain after the transaction was committed.
    fn send_msgs(&self, proto_msgs: Vec<prost_types::Any>) -> Result<Vec<Event>, Error>;

    fn get_signer(&self) -> Result<String, Error>;

    fn get_key(&self) -> Result<KeyEntry, Error>;

    fn query_events_from_txs(&self, request: QueryTxRequest) -> Result<Vec<Event>, Error>;
}

impl Serialize for dyn ChainHandle {
    fn serialize<S>(&self, serializer: S) -> Result<<S as Serializer>::Ok, <S as Serializer>::Error>
    where
        S: Serializer,
    {
        self.id().serialize(serializer)
    }
}
