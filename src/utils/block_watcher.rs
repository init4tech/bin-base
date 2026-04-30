//! Rollup chain block watcher that subscribes to new blocks and tracks the
//! current rollup block number.

use alloy::{
    network::Ethereum,
    providers::{Provider, RootProvider},
    transports::TransportError,
};
use tokio::{
    sync::{broadcast::error::RecvError, watch},
    task::JoinHandle,
};
use tracing::{debug, error, trace};

/// Rollup chain block watcher that subscribes to new blocks and broadcasts
/// updates via a watch channel.
#[derive(Debug)]
pub struct BlockWatcher {
    /// Watch channel responsible for broadcasting block number updates.
    block_number: watch::Sender<u64>,

    /// Rollup chain provider.
    rollup_provider: RootProvider<Ethereum>,
}

impl BlockWatcher {
    /// Creates a new [`BlockWatcher`] with the given provider and initial
    /// block number.
    pub fn new(rollup_provider: RootProvider<Ethereum>, initial: u64) -> Self {
        Self {
            block_number: watch::channel(initial).0,
            rollup_provider,
        }
    }

    /// Creates a new [`BlockWatcher`], fetching the current block number first.
    pub async fn with_current_block(
        rollup_provider: RootProvider<Ethereum>,
    ) -> Result<Self, TransportError> {
        let block_number = rollup_provider.get_block_number().await?;
        Ok(Self::new(rollup_provider, block_number))
    }

    /// Subscribe to block number updates.
    pub fn subscribe(&self) -> SharedBlockNumber {
        self.block_number.subscribe().into()
    }

    /// Spawns the block watcher task.
    pub fn spawn(self) -> (SharedBlockNumber, JoinHandle<()>) {
        (self.subscribe(), tokio::spawn(self.task_future()))
    }

    async fn task_future(self) {
        let mut sub = match self.rollup_provider.subscribe_blocks().await {
            Ok(sub) => sub,
            Err(error) => {
                error!(%error);
                return;
            }
        };

        debug!("subscribed to rollup chain blocks");

        loop {
            match sub.recv().await {
                Ok(header) => {
                    let block_number = header.number;
                    self.block_number.send_replace(block_number);
                    trace!(block_number, "updated rollup block number");
                }
                Err(RecvError::Lagged(missed)) => {
                    debug!(%missed, "block subscription lagged");
                }
                Err(RecvError::Closed) => {
                    debug!("block subscription closed");
                    break;
                }
            }
        }
    }
}

/// A shared block number, wrapped in a [`tokio::sync::watch`] Receiver.
///
/// The block number is periodically updated by a [`BlockWatcher`] task, and
/// can be read or awaited for changes. This allows multiple tasks to observe
/// block number updates.
#[derive(Debug, Clone)]
pub struct SharedBlockNumber(watch::Receiver<u64>);

impl From<watch::Receiver<u64>> for SharedBlockNumber {
    fn from(inner: watch::Receiver<u64>) -> Self {
        Self(inner)
    }
}

impl SharedBlockNumber {
    /// Get the current block number.
    pub fn get(&self) -> u64 {
        *self.0.borrow()
    }

    /// Wait for the block number to change, then return the new value.
    ///
    /// This is implemented using [`Receiver::changed`].
    ///
    /// [`Receiver::changed`]: tokio::sync::watch::Receiver::changed
    pub async fn changed(&mut self) -> Result<u64, watch::error::RecvError> {
        self.0.changed().await?;
        Ok(*self.0.borrow_and_update())
    }

    /// Wait for the block number to reach at least `target`.
    ///
    /// Returns the block number once it is >= `target`.
    pub async fn wait_until(&mut self, target: u64) -> Result<u64, watch::error::RecvError> {
        self.0.wait_for(|&n| n >= target).await.map(|r| *r)
    }
}
