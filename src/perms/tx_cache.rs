use crate::perms::oauth::SharedToken;
use serde::de::DeserializeOwned;
use signet_tx_cache::{
    error::Result,
    types::{BundleKey, CacheObject, TxCacheBundle, TxCacheBundleResponse, TxCacheBundlesResponse},
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
        let secret = self.token.secret().await.unwrap_or_else(|_| {
            warn!("Failed to get token secret");
            "".to_string()
        });

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
    #[instrument(skip_all)]
    pub async fn get_bundles(&self, query: Option<BundleKey>) -> Result<Vec<TxCacheBundle>> {
        self.get_inner_with_token::<TxCacheBundlesResponse>(BUNDLES, query)
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
        self.get_inner_with_token::<TxCacheBundleResponse>(&url, None)
            .await
            .map(|response| response.bundle)
    }
}
