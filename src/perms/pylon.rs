use crate::perms::oauth::SharedToken;
use alloy::consensus::TxEnvelope;
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
    /// * `sidecar` - The blob transaction sidecar ([`BlobTransactionSidecarEip7594`]).
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The sidecar format is invalid ([`PylonError::InvalidSidecar`])
    /// - A sidecar already exists for this transaction hash ([`PylonError::SidecarAlreadyExists`])
    /// - An internal server error occurred ([`PylonError::InternalError`])
    /// - A network error occurred ([`PylonError::Request`])
    ///
    /// [`B256`]: <https://docs.rs/alloy/latest/alloy/primitives/aliases/type.B256.html>
    /// [`BlobTransactionSidecarEip7594`]: <https://docs.rs/alloy/latest/alloy/consensus/struct.BlobTransactionSidecarEip7594.html>
    #[instrument(skip_all)]
    pub async fn post_sidecar(&self, tx: TxEnvelope) -> Result<(), PylonError> {
        // verify that the sidecar is in EIP-7594 format
        let is_eip7594 = tx
            .as_eip4844()
            .and_then(|tx| tx.tx().sidecar().map(|v| v.is_eip7594()));
        if is_eip7594 != Some(true) {
            return Err(PylonError::InvalidSidecar(
                "sidecar is not in EIP-7594 format".to_string(),
            ));
        }

        let tx_hash = tx.hash();
        let url = self.url.join(&format!("v2/sidecar/{tx_hash}"))?;
        let secret = self
            .token
            .secret()
            .await
            .map_err(PylonError::MissingAuthToken)?;

        let response = self
            .client
            .post(url)
            .json(&tx)
            .bearer_auth(secret)
            .send()
            .await?;

        match response.status() {
            reqwest::StatusCode::OK => Ok(()),
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
