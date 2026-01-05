//! Host chain block watcher that subscribes to new blocks and tracks the
//! current host block number.

use alloy::{
    network::Ethereum,
    providers::{Provider, RootProvider},
    transports::TransportError,
};
use tokio::{sync::watch, task::JoinHandle};
use tokio_stream::StreamExt;
use tracing::{debug, error, trace};

/// Errors that can occur on the [`BlockWatcher`] task.
#[derive(Debug, thiserror::Error)]
pub enum BlockWatcherError {
    /// Failed to subscribe to host chain blocks.
    #[error("failed to subscribe to host chain blocks: {0}")]
    SubscribeError(TransportError),
}

impl From<TransportError> for BlockWatcherError {
    fn from(err: TransportError) -> Self {
        BlockWatcherError::SubscribeError(err)
    }
}

/// Host chain block watcher that subscribes to new blocks and broadcasts
/// updates via a watch channel.
#[derive(Debug)]
pub struct BlockWatcher {
    /// Watch channel responsible for broadcasting block number updates.
    block_number: watch::Sender<u64>,

    /// Host chain provider.
    host_provider: RootProvider<Ethereum>,
}

impl BlockWatcher {
    /// Creates a new [`BlockWatcher`] with the given provider and initial
    /// block number.
    pub fn new(host_provider: RootProvider<Ethereum>, initial: u64) -> Self {
        Self {
            block_number: watch::channel(initial).0,
            host_provider,
        }
    }

    /// Creates a new [`BlockWatcher`], fetching the current block number first.
    pub async fn with_current_block(
        host_provider: RootProvider<Ethereum>,
    ) -> Result<Self, BlockWatcherError> {
        let block_number = host_provider.get_block_number().await?;
        Ok(Self::new(host_provider, block_number))
    }

    /// Subscribe to block number updates.
    pub fn subscribe(&self) -> watch::Receiver<u64> {
        self.block_number.subscribe()
    }

    /// Spawns the block watcher task.
    pub fn spawn(self) -> JoinHandle<()> {
        tokio::spawn(self.task_future())
    }

    async fn task_future(self) {
        let sub = match self.host_provider.subscribe_blocks().await {
            Ok(sub) => sub,
            Err(err) => {
                error!(error = ?err, "failed to subscribe to host chain blocks");
                return;
            }
        };
        let mut stream = sub.into_stream();

        debug!("subscribed to host chain blocks");

        while let Some(header) = stream.next().await {
            let block_number = header.number;
            self.block_number.send_replace(block_number);
            trace!(block_number, "updated host block number");
        }
    }
}
