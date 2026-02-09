use crate::perms::oauth::SharedToken;
use alloy::eips::eip2718::Eip2718Error;
use thiserror::Error;
use tracing::instrument;

/// Errors that can occur when interacting with the Pylon API.
#[derive(Debug, Error)]
pub enum PylonError {
    /// Invalid sidecar format (400).
    #[error("invalid sidecar: {0}")]
    InvalidSidecar(String),

    /// Sidecar already exists for this transaction (409).
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

    /// Missing auth token.
    #[error("missing auth token")]
    MissingAuthToken(tokio::sync::watch::error::RecvError),

    /// Invalid transaction bytes.
    #[error("invalid transaction bytes: {0}")]
    InvalidTransactionBytes(Eip2718Error),
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

    /// Post a blob transaction to the Pylon server.
    ///
    /// The transaction must be an EIP-4844 blob transaction with an EIP-7594
    /// sidecar attached. Non-EIP-7594 sidecars will be rejected.
    ///
    /// # Arguments
    ///
    /// * `tx` - The raw EIP-2718 encoded transaction bytes ([`Bytes`]).
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The transaction bytes are invalid ([`PylonError::InvalidTransactionBytes`])
    /// - The sidecar is missing or not in EIP-7594 format ([`PylonError::InvalidSidecar`])
    /// - A sidecar already exists for this transaction hash ([`PylonError::SidecarAlreadyExists`])
    /// - An internal server error occurred ([`PylonError::InternalError`])
    /// - A network error occurred ([`PylonError::Request`])
    ///
    /// [`Bytes`]: https://docs.rs/alloy/latest/alloy/primitives/struct.Bytes.html
    #[instrument(skip_all)]
    pub async fn post_blob_tx(&self, raw_tx: alloy::primitives::Bytes) -> Result<(), PylonError> {
        let url = self.url.join("v2/sidecar")?;
        let secret = self
            .token
            .secret()
            .await
            .map_err(PylonError::MissingAuthToken)?;

        let response = self
            .client
            .post(url)
            .header(reqwest::header::CONTENT_TYPE, "application/octet-stream")
            .body(raw_tx.0)
            .bearer_auth(secret)
            .send()
            .await?;

        match response.status() {
            reqwest::StatusCode::CREATED => Ok(()),
            reqwest::StatusCode::BAD_REQUEST => {
                let text = response.text().await.unwrap_or_default();
                Err(PylonError::InvalidSidecar(text))
            }
            reqwest::StatusCode::CONFLICT => Err(PylonError::SidecarAlreadyExists),
            reqwest::StatusCode::INTERNAL_SERVER_ERROR => {
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
