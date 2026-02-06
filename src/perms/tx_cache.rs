use crate::perms::oauth::SharedToken;
use serde::de::DeserializeOwned;
use signet_tx_cache::{
    error::TxCacheError,
    types::{BundleKey, CacheObject, TxCacheBundle, TxCacheBundleResponse, TxCacheBundlesResponse},
    TxCache,
};
use thiserror::Error;
use tokio::sync::watch;
use tracing::instrument;

/// Result type for [`BuilderTxCache`] operations.
pub type Result<T> = std::result::Result<T, BuilderTxCacheError>;

/// Errors that can occur when using the [`BuilderTxCache`] client.
#[derive(Debug, Error)]
pub enum BuilderTxCacheError {
    /// Failed to retrieve the authentication token.
    #[error("failed to retrieve auth token: {0}")]
    TokenRetrieval(#[from] watch::error::RecvError),

    /// An error occurred during a TxCache operation.
    #[error(transparent)]
    TxCache(#[from] TxCacheError),
}

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
    /// Instantiate with the given transaction cache and shared token.
    pub fn new(url: reqwest::Url, token: SharedToken) -> Self {
        Self {
            tx_cache: TxCache::new(url),
            token,
        }
    }

    /// Instantiate from a string URL and shared token.
    pub fn new_from_string(url: &str, token: SharedToken) -> Result<Self> {
        let tx_cache = TxCache::new_from_string(url)?;
        Ok(Self { tx_cache, token })
    }

    /// Instantiate with the given transaction cache and shared token, using
    /// a specific reqwest client.
    pub const fn new_with_client(
        url: reqwest::Url,
        client: reqwest::Client,
        token: SharedToken,
    ) -> Self {
        Self {
            tx_cache: TxCache::new_with_client(url, client),
            token,
        }
    }

    /// Get a reference to the transaction cache client.
    pub const fn inner(&self) -> &TxCache {
        &self.tx_cache
    }

    /// Get a reference to the shared token.
    pub const fn token(&self) -> &SharedToken {
        &self.token
    }

    async fn get_inner_with_token<T>(&self, join: &str, query: Option<T::Key>) -> Result<T>
    where
        T: DeserializeOwned + CacheObject,
    {
        let url = self.tx_cache.url().join(join)?;
        let secret = self.token.secret().await?;

        self.tx_cache
            .client()
            .get(url)
            .query(&query)
            .bearer_auth(secret)
            .send()
            .await?
            .error_for_status()?
            .json::<T>()
            .await
            .map_err(Into::into)
    }

    /// Get bundles from the cache.
    ///
    /// # Arguments
    ///
    /// * `query` - Optional pagination parameters. Pass `None` to retrieve the
    ///   first page of bundles. Pass `Some(BundleKey)` with pagination tokens
    ///   from the previous response to retrieve subsequent pages of results.
    ///
    /// # Returns
    ///
    /// A response containing bundles for the current page and an
    /// optional pagination key. If the pagination key is present, there are
    /// more pages available. Pass this key to subsequent calls to retrieve
    /// the next page.
    ///
    /// Returns an error if the request fails or the builder is not permissioned
    /// for the current slot.
    #[instrument(skip_all)]
    pub async fn get_bundles(&self, query: Option<BundleKey>) -> Result<TxCacheBundlesResponse> {
        self.get_inner_with_token::<TxCacheBundlesResponse>(BUNDLES, query)
            .await
    }

    fn get_bundle_url_path(&self, bundle_id: &str) -> String {
        format!("{BUNDLES}/{bundle_id}")
    }

    /// Get a bundle from the cache by its UUID. For convenience, this method
    /// takes a string reference, which is expected to be a valid UUID.
    #[instrument(skip_all)]
    pub async fn get_bundle(&self, bundle_id: &str) -> Result<TxCacheBundle> {
        let url = self.get_bundle_url_path(bundle_id);
        self.get_inner_with_token::<TxCacheBundleResponse>(&url, None)
            .await
            .map(|response| response.bundle)
    }
}
