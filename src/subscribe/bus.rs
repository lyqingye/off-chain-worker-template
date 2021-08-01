use std::collections::VecDeque;

use crossbeam_channel as channel;

pub struct EventBus<T> {
    txs: VecDeque<channel::Sender<T>>,
}

impl<T> Default for EventBus<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> EventBus<T> {
    pub fn new() -> Self {
        Self {
            txs: VecDeque::new(),
        }
    }

    pub fn subscribe(&mut self) -> channel::Receiver<T> {
        let (tx, rx) = channel::unbounded();
        self.txs.push_back(tx);
        rx
    }

    pub fn broadcast(&mut self, value: T)
    where
        T: Clone,
    {
        let mut disconnected = Vec::new();

        for (idx, tx) in self.txs.iter().enumerate() {
            // TODO: Avoid cloning when sending to last subscriber
            if let Err(channel::SendError(_)) = tx.send(value.clone()) {
                disconnected.push(idx);
            }
        }

        // Remove all disconnected subscribers
        for idx in disconnected {
            self.txs.remove(idx);
        }
    }
}

