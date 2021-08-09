
use std::fmt::Debug;

use crossbeam_channel as channel;
use super::{reply_channel, ChainHandle, ChainRequest, ReplyTo, Subscription};
use tendermint::chain::Id as ChainId;
use crate::{error::{Error, Kind}, events::Event};

#[derive(Debug, Clone)]
pub struct ProdChainHandle {
    /// Chain identifier
    chain_id: ChainId,

    /// The handle's channel for sending requests to the runtime
    runtime_sender: channel::Sender<ChainRequest>,
}

impl ProdChainHandle {
    pub fn new(chain_id: ChainId, sender: channel::Sender<ChainRequest>) -> Self {
        Self {
            chain_id,
            runtime_sender: sender,
        }
    }

    /// Sending requests to the runtime
    fn send<F, O>(&self, f: F) -> Result<O, Error>
    where
        F: FnOnce(ReplyTo<O>) -> ChainRequest,
        O: Debug,
    {
        let (sender, receiver) = reply_channel();
        let input = f(sender);

        self.runtime_sender
            .send(input)
            .map_err(|e| Kind::Channel.context(e))?;

        receiver.recv().map_err(|e| Kind::Channel.context(e))?
    }
}

impl ChainHandle for ProdChainHandle {

    fn id(&self) -> ChainId {
        self.chain_id.clone()
    }

    /// Sending shutdown requests to the runtime
    fn shutdown(&self) -> Result<(), Error> {
        self.send(|reply_to| ChainRequest::Shutdown { reply_to })
    }

    /// Sending proto messages to the runtime
    fn send_msgs(&self, proto_msgs: Vec<prost_types::Any>) -> Result<Vec<Event>, Error> {
        self.send(|reply_to| ChainRequest::SendMsgs { proto_msgs, reply_to })
    }
}
