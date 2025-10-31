use crate::perms::oauth::SharedToken;
use eyre::Result;
use serde::de::DeserializeOwned;
use signet_tx_cache::{
    client::TxCache,
    types::{TxCacheBundle, TxCacheBundleResponse, TxCacheBundlesResponse},
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
        let secret = self.token.secret().await?;

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

    async fn get_inner_with_query_and_token<T>(
        &self,
        join: &'static str,
        query: PaginationParams,
    ) -> Result<T, Error>
    where
        T: DeserializeOwned,
    {
        // Append the path to the URL.
        let secret = self.token.secret().await?;
        let url = self
            .url
            .join(join)
            .inspect_err(|e| warn!(%e, "Failed to join URL. Not querying transaction cache."))?;

        let mut request = self.client.get(url);

        if let Some(cursor) = query.cursor() {
            request = request.query(&[("cursor", cursor)]);
        }
        if let Some(limit) = query.limit() {
            request = request.query(&[("limit", limit)]);
        }

        request
            .bearer_auth(secret)
            .send()
            .await
            .inspect_err(|e| warn!(%e, "Failed to get object from transaction cache."))?
            .json::<T>()
            .await
            .map_err(Into::into)
    }

    /// Get bundles from the cache.
    #[instrument(skip_all)]
    pub async fn get_bundles(&self, query: Option<PaginationParams>) -> Result<Vec<TxCacheBundle>> {
        if let Some(query) = query {
            self.get_inner_with_query_and_token::<TxCacheBundlesResponse>(BUNDLES, query)
                .await
                .map(|response| response.bundles)
        } else {
            self.get_inner_with_token::<TxCacheBundlesResponse>(BUNDLES)
                .await
                .map(|response| response.bundles)
        }
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
