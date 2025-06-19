use serde::de::DeserializeOwned;
use signet_tx_cache::{client::TxCache, types::{TxCacheBundle, TxCacheBundleResponse, TxCacheBundlesResponse}};
use eyre::{bail, Error};
use tracing::{instrument, warn};
use crate::perms::oauth::{SharedToken};

const BUNDLES: &str = "bundles";

pub struct TxCacheClient {
    /// The transaction cache client.
    pub tx_cache: TxCache,
    /// The shared token for authentication.
    pub token: SharedToken,
}

impl TxCacheClient {
    /// Create a new `TxCacheClient` with the given transaction cache and shared token.
    pub fn new(tx_cache: TxCache, token: SharedToken) -> Self {
        Self { tx_cache, token }
    }

    /// Get a reference to the transaction cache client.
    pub fn tx_cache(&self) -> &TxCache {
        &self.tx_cache
    }

    /// Get a reference to the shared token.
    pub fn token(&self) -> &SharedToken {
        &self.token
    }

    async fn get_inner_with_token<T: DeserializeOwned>(&self, join: &str) -> Result<T, Error> {
        let url = self.tx_cache.url().join(join)?;
        let Some(token) = self.token.read() else {
            bail!("No token available for authentication");
        };
        
        self.tx_cache.client().get(url)
            .bearer_auth(token.access_token().secret())
            .send()
            .inspect_err(|e| warn!(%e, "Failed to get object from transaction cache"))?
            .json::<T>()
            .await
            .map_err(Into::into)
    }

    /// Get bundles from the cache.
    #[instrument(skip_all)]
    pub async fn get_bundles(&self) -> Result<Vec<TxCacheBundle>, Error> {
        let response: TxCacheBundlesResponse =
            self.get_inner_with_token::<TxCacheBundlesResponse>(BUNDLES).await?;
        Ok(response.bundles)
    }

    /// Get a bundle from the cache.
    #[instrument(skip_all)]
    pub async fn get_bundle(&self) -> Result<TxCacheBundle, Error> {
        let response: TxCacheBundleResponse =
            self.get_inner_with_token::<TxCacheBundleResponse>(BUNDLES).await?;
        Ok(response.bundle)
    }
}

