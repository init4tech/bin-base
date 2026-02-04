use crate::perms::oauth::SharedToken;
use alloy::{
    consensus::{BlobTransactionSidecarVariant, EnvKzgSettings},
    primitives::B256,
};
use thiserror::Error;
use tracing::{instrument, warn};

/// Errors that can occur when interacting with the Pylon API.
#[derive(Debug, Error)]
pub enum PylonError {
    /// Invalid sidecar format (400).
    #[error("invalid sidecar: {0}")]
    InvalidSidecar(String),

    /// Sidecar already exists for this transaction hash (409).
    #[error("sidecar already exists")]
    SidecarAlreadyExists,

    /// Internal server error (500).
    #[error("internal server error: {0}")]
    InternalError(String),

    /// Request error.
    #[error("request error: {0}")]
    Request(#[from] reqwest::Error),

    /// URL parse error.
    #[error("URL parse error: {0}")]
    UrlParse(#[from] url::ParseError),

    /// KZG conversion error when converting EIP-4844 to EIP-7594.
    #[error("KZG conversion error: {0}")]
    KzgConversion(String),
}

/// A client for interacting with the Pylon blob server API.
#[derive(Debug, Clone)]
pub struct PylonClient {
    /// The reqwest client.
    client: reqwest::Client,
    /// The base URL of the Pylon server.
    url: reqwest::Url,
    /// The shared token for authentication.
    token: SharedToken,
}

impl PylonClient {
    /// Instantiate with the given URL and shared token.
    pub fn new(url: reqwest::Url, token: SharedToken) -> Self {
        Self {
            client: reqwest::Client::new(),
            url,
            token,
        }
    }

    /// Instantiate from a string URL and shared token.
    pub fn new_from_string(url: &str, token: SharedToken) -> Result<Self, PylonError> {
        let url = url.parse()?;
        Ok(Self::new(url, token))
    }

    /// Instantiate with a custom reqwest client.
    pub const fn new_with_client(
        url: reqwest::Url,
        client: reqwest::Client,
        token: SharedToken,
    ) -> Self {
        Self { client, url, token }
    }

    /// Get a reference to the base URL.
    pub const fn url(&self) -> &reqwest::Url {
        &self.url
    }

    /// Get a reference to the reqwest client.
    pub const fn client(&self) -> &reqwest::Client {
        &self.client
    }

    /// Get a reference to the shared token.
    pub const fn token(&self) -> &SharedToken {
        &self.token
    }

    /// Post a blob transaction sidecar to the Pylon server.
    ///
    /// If the sidecar is in EIP-4844 format, it will be converted to EIP-7594
    /// format before posting.
    ///
    /// # Arguments
    ///
    /// * `tx_hash` - The transaction hash ([`B256`]).
    /// * `sidecar` - The blob transaction sidecar ([`BlobTransactionSidecarVariant`]).
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The sidecar format is invalid ([`PylonError::InvalidSidecar`])
    /// - A sidecar already exists for this transaction hash ([`PylonError::SidecarAlreadyExists`])
    /// - An internal server error occurred ([`PylonError::InternalError`])
    /// - The KZG conversion from EIP-4844 to EIP-7594 failed ([`PylonError::KzgConversion`])
    /// - A network error occurred ([`PylonError::Request`])
    ///
    /// [`B256`]: https://docs.rs/alloy/latest/alloy/primitives/aliases/type.B256.html
    /// [`BlobTransactionSidecarVariant`]: https://docs.rs/alloy/latest/alloy/consensus/transaction/eip4844/enum.BlobTransactionSidecarVariant.html
    #[instrument(skip_all)]
    pub async fn post_sidecar(
        &self,
        tx_hash: B256,
        sidecar: BlobTransactionSidecarVariant,
    ) -> Result<(), PylonError> {
        // Convert to EIP-7594 if necessary
        let sidecar = match sidecar {
            BlobTransactionSidecarVariant::Eip4844(s) => {
                let converted = s
                    .try_into_7594(EnvKzgSettings::Default.get())
                    .map_err(|e| PylonError::KzgConversion(e.to_string()))?;
                BlobTransactionSidecarVariant::Eip7594(converted)
            }
            eip7594 @ BlobTransactionSidecarVariant::Eip7594(_) => eip7594,
        };

        let url = self.url.join(&format!("v2/sidecar/{tx_hash}"))?;
        let secret = self.token.secret().await.unwrap_or_else(|_| {
            warn!("Failed to get token secret");
            "".to_string()
        });

        let response = self
            .client
            .post(url)
            .json(&sidecar)
            .bearer_auth(secret)
            .send()
            .await?;

        match response.status() {
            status if status.is_success() => Ok(()),
            status if status == reqwest::StatusCode::BAD_REQUEST => {
                let text = response.text().await.unwrap_or_default();
                Err(PylonError::InvalidSidecar(text))
            }
            status if status == reqwest::StatusCode::CONFLICT => {
                Err(PylonError::SidecarAlreadyExists)
            }
            status if status.is_server_error() => {
                let text = response.text().await.unwrap_or_default();
                Err(PylonError::InternalError(text))
            }
            _ => {
                response.error_for_status()?;
                Ok(())
            }
        }
    }
}
