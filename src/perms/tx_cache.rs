use crate::perms::oauth::SharedToken;
use serde::de::DeserializeOwned;
use signet_tx_cache::{
    error::Result,
    types::{TxCacheBundle, TxCacheBundleResponse, TxCacheBundlesResponse},
    TxCache,
};
use tracing::{instrument, warn};

const BUNDLES: &str = "bundles";

/// A client for interacting with the transaction cache, a thin wrapper around
/// the [`TxCache`] and [`SharedToken`] that implements the necessary methods
/// to fetch bundles and bundle details.
#[derive(Debug, Clone)]
pub struct BuilderTxCache {
    /// The transaction cache client.
    tx_cache: TxCache,
    /// The shared token for authentication.
    token: SharedToken,
}

impl std::ops::Deref for BuilderTxCache {
    type Target = TxCache;

    fn deref(&self) -> &Self::Target {
        &self.tx_cache
    }
}

impl std::ops::DerefMut for BuilderTxCache {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.tx_cache
    }
}

impl BuilderTxCache {
    /// Create a new `TxCacheClient` with the given transaction cache and shared token.
    pub const fn new(tx_cache: TxCache, token: SharedToken) -> Self {
        Self { tx_cache, token }
    }

    /// Get a reference to the transaction cache client.
    pub const fn tx_cache(&self) -> &TxCache {
        &self.tx_cache
    }

    /// Get a reference to the shared token.
    pub const fn token(&self) -> &SharedToken {
        &self.token
    }

    async fn get_inner_with_token<T: DeserializeOwned>(&self, join: &str) -> Result<T> {
        let url = self.tx_cache.url().join(join)?;
        let secret = self.token.secret().await.unwrap_or_else(|_| {
            warn!("Failed to get token secret");
            "".to_string()
        });

        self.tx_cache
            .client()
            .get(url)
            .bearer_auth(secret)
            .send()
            .await
            .inspect_err(|e| warn!(%e, "Failed to get object from transaction cache"))?
            .json::<T>()
            .await
            .map_err(Into::into)
    }

    /// Get bundles from the cache.
    #[instrument(skip_all)]
    pub async fn get_bundles(&self) -> Result<Vec<TxCacheBundle>> {
        self.get_inner_with_token::<TxCacheBundlesResponse>(BUNDLES)
            .await
            .map(|response| response.bundles)
    }

    fn get_bundle_url_path(&self, bundle_id: &str) -> String {
        format!("{BUNDLES}/{bundle_id}")
    }

    /// Get a bundle from the cache by its UUID. For convenience, this method
    /// takes a string reference, which is expected to be a valid UUID.
    #[instrument(skip_all)]
    pub async fn get_bundle(&self, bundle_id: &str) -> Result<TxCacheBundle> {
        let url = self.get_bundle_url_path(bundle_id);
        self.get_inner_with_token::<TxCacheBundleResponse>(&url)
            .await
            .map(|response| response.bundle)
    }
}
