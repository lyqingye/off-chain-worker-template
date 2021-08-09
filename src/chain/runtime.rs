use std::{sync::Arc, thread};

use crate::chain::Chain;
use crate::config::ChainConfig;
use crate::error::{Error, Kind};
use crate::events::Event;
use crate::subscribe::monitor::{EventReceiver, TxMonitorCmd, MonitorCmd};
use tokio::runtime::Runtime as TokioRuntime;

use super::handle::prod::ProdChainHandle;
use super::handle::{ChainHandle, ChainRequest, ReplyTo, Subscription};
use tracing::error;

pub struct Threads {
    pub chain_runtime: thread::JoinHandle<()>,
    pub event_monitor: Option<thread::JoinHandle<()>>,
}

pub struct ChainRuntime<C: Chain> {
    /// The specific chain this runtime runs against
    chain: C,

    /// The sender side of a channel to this runtime. Any `ChainHandle` can use this to send
    /// chain requests to this runtime
    request_sender: crossbeam_channel::Sender<ChainRequest>,

    /// The receiving side of a channel to this runtime. The runtime consumes chain requests coming
    /// in through this channel.
    request_receiver: crossbeam_channel::Receiver<ChainRequest>,

    /// Receiver channel from the event bus
    event_receiver: EventReceiver,

    /// Sender channel to terminate the event monitor
    tx_monitor_cmd: TxMonitorCmd,

    #[allow(dead_code)]
    rt: Arc<TokioRuntime>,
}

impl <C: Chain + Send + 'static> ChainRuntime<C> {

    pub fn spawn(
        config: ChainConfig,
        rt: Arc<TokioRuntime>,
    ) -> Result<Box<dyn ChainHandle>, Error> {

        let chain = C::bootstrap(config, rt.clone())?;

        // Start the event monitor
        let (event_batch_tx, tx_monidor_cmd) = chain.init_event_monitor(rt.clone())?;

        // Instantiate & spawn the runtime
        let (handle,_) = Self::init(chain, event_batch_tx, tx_monidor_cmd, rt);

        Ok(handle)
    }


    /// Initializes a runtime for given chain, and spawns the associated thread
    fn init(
        chain: C,
        event_receiver: EventReceiver,
        tx_monitor_cmd: TxMonitorCmd,
        rt: Arc<TokioRuntime>,
    ) -> (Box<dyn ChainHandle>, thread::JoinHandle<()>) {
        let chain_runtime = Self::new(chain, event_receiver, tx_monitor_cmd, rt);

        let handle = chain_runtime.handle();

        let id = handle.id();
        let thread = thread::spawn(move || {
            if let Err(e) = chain_runtime.run() {
                error!("failed to start runtime for chain '{}': {}", id, e);
            }
        });

        (handle, thread)
    }

    /// Basic constructor
    fn new(
        chain: C,
        event_receiver: EventReceiver,
        tx_monitor_cmd: TxMonitorCmd,
        rt: Arc<TokioRuntime>,
    ) -> Self {
        let (request_sender, request_receiver) = crossbeam_channel::unbounded::<ChainRequest>();
        Self {
            chain,
            request_sender,
            request_receiver,
            event_receiver,
            tx_monitor_cmd,
            rt,
        }
    }

    pub fn handle(&self) -> Box<dyn ChainHandle> {
        let chain_id = self.chain.id().clone();
        let sender = self.request_sender.clone();

        Box::new(ProdChainHandle::new(chain_id, sender))
    }

    /// Runtime event loop
    fn run(mut self) -> Result<(), Error> {
        loop {
            crossbeam_channel::select! {

                // Process Event log
                recv(self.event_receiver) -> event_batch => {
                    match event_batch {
                        Ok(event_batch) => {
                        },
                        Err(e) => {
                            error!("received error via event bus: {}", e);
                            return Err(Kind::Channel.into());
                        },
                    }
                },

                // Process chain requests
                recv(self.request_receiver) -> event => {
                    match event {
                        // Shudown the chain and shutdown the event monitor
                        Ok(ChainRequest::Shutdown { reply_to }) => {
                            self.tx_monitor_cmd.send(MonitorCmd::Shutdown).map_err(Kind::channel)?;

                            let res = self.chain.shutdown();
                            reply_to.send(res).map_err(Kind::channel)?;

                            break;
                        }

                        Ok(ChainRequest::SendMsgs { proto_msgs, reply_to }) => {
                            self.send_msgs(proto_msgs, reply_to)?
                        },

                        Ok(ChainRequest::Subscribe { reply_to }) => {
                            self.subscribe(reply_to)?
                        },

                        // Ok(ChainRequest::Signer { reply_to }) => {
                        //     self.get_signer(reply_to)?
                        // }

                        // Ok(ChainRequest::Key { reply_to }) => {
                        //     self.get_key(reply_to)?
                        // }

                        Err(e) => error!("received error via chain request channel: {}", e),
                    }
                },
            }
        }

        Ok(())
    }

    fn send_msgs(
        &mut self,
        proto_msgs: Vec<prost_types::Any>,
        reply_to: ReplyTo<Vec<Event>>,
    ) -> Result<(), Error> {
        let result = self.chain.send_msgs(proto_msgs);

        reply_to.send(result).map_err(Kind::channel)?;

        Ok(())
    }

    fn subscribe(
        &mut self,
        reply_to: ReplyTo<Subscription>,
    ) -> Result<(), Error> {

        Ok(())
    }
}
