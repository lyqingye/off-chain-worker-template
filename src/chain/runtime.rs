use std::{sync::Arc, thread};

use crossbeam_channel as channel;
use tokio::runtime::Runtime as TokioRuntime;
use tracing::error;
use crate::subscribe::monitor::{EventReceiver,TxMonitorCmd};
use crate::chain::Chain;

pub struct Threads {
    pub chain_runtime: thread::JoinHandle<()>,
    pub event_monitor: Option<thread::JoinHandle<()>>,
}

pub struct ChainRuntime<C: Chain> {
    /// The specific chain this runtime runs against
    chain: C,

    /// The sender side of a channel to this runtime. Any `ChainHandle` can use this to send
    /// chain requests to this runtime
    /// request_sender: channel::Sender<ChainRequest>,

    /// The receiving side of a channel to this runtime. The runtime consumes chain requests coming
    /// in through this channel.
    /// request_receiver: channel::Receiver<ChainRequest>,

    /// Receiver channel from the event bus
    event_receiver: EventReceiver,

    /// Sender channel to terminate the event monitor
    tx_monitor_cmd: TxMonitorCmd,

    #[allow(dead_code)]
    rt: Arc<TokioRuntime>,
}



