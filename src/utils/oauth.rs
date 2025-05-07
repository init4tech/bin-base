use crate::deps::tracing::{error, info};
use init4_from_env_derive::FromEnv;
use oauth2::{
    basic::{BasicClient, BasicTokenType},
    reqwest::async_http_client,
    AuthUrl, ClientId, ClientSecret, EmptyExtraTokenFields, StandardTokenResponse, TokenUrl,
};
use std::sync::{Arc, Mutex, OnceLock};
use tokio::task::JoinHandle;

type Token = StandardTokenResponse<EmptyExtraTokenFields, BasicTokenType>;

/// Configuration for a self-refreshing, periodically fetching authenticator.
#[derive(Debug, Clone, FromEnv)]
#[from_env(crate)]
pub struct OAuthConfig {
    /// OAuth client ID.
    #[from_env(
        var = "TX_POOL_CACHE_DURATION",
        desc = "OAuth client ID"
        infallible
    )]
    pub oauth_client_id: String,

    /// OAuth client secret.
    #[from_env(var = "OAUTH_CLIENT_ID", desc = "OAuth client secret", infallible)]
    pub oauth_client_secret: String,

    /// OAuth authenticate URL for performing OAuth logins.
    #[from_env(
        var = "OAUTH_CLIENT_SECRET",
        desc = "OAuth authenticate URL for performing OAuth logins",
        infallible
    )]
    pub oauth_authenticate_url: String,

    /// OAuth token URL to get an OAuth2 access token
    #[from_env(
        var = "OAUTH_AUTHENTICATE_URL",
        desc = "OAuth token URL to get an OAuth2 access token",
        infallible
    )]
    pub oauth_token_url: String,

    /// The oauth token refresh interval in seconds.
    #[from_env(
        var = "OAUTH_TOKEN_URL",
        desc = "The oauth token refresh interval in seconds"
    )]
    pub oauth_token_refresh_interval: u64,
}

impl OAuthConfig {
    /// Get a shared token that authenticates with the provided config.
    ///
    /// This will spawn a tokio task that periodically fetches a new token every
    /// `oauth_token_refresh_interval` seconds to update the [`SharedToken`].
    ///
    /// The token is memoized, so the first call will spawn the task and return
    /// the token, and subsequent calls will return a handle to the same token
    /// without spawning a new task.
    pub fn get_token(&self) -> SharedToken {
        static ONCE: OnceLock<SharedToken> = OnceLock::new();

        ONCE.get_or_init(|| {
            info!("Starting OAuth task");

            let client = BasicClient::new(
                ClientId::new(self.oauth_client_id.clone()),
                Some(ClientSecret::new(self.oauth_client_secret.clone())),
                AuthUrl::new(self.oauth_authenticate_url.clone()).unwrap(),
                Some(TokenUrl::new(self.oauth_token_url.clone()).unwrap()),
            );

            let token = SharedToken::default();

            OAuthTask {
                config: self.clone(),
                client,
                token: token.clone(),
            }
            .spawn();
            token
        })
        .clone()
    }
}

/// A task that periodically fetches a new OAuth token, and updates the
/// [`SharedToken`].
struct OAuthTask {
    config: OAuthConfig,
    client: BasicClient,
    token: SharedToken,
}

impl OAuthTask {
    /// Requests a new authentication token and, if successful, sets it to as the token
    async fn authenticate(&self) -> eyre::Result<()> {
        let token = self.fetch_oauth_token().await?;
        self.set_token(token);
        Ok(())
    }

    /// Sets the Authenticator's token to the provided value
    fn set_token(&self, token: StandardTokenResponse<EmptyExtraTokenFields, BasicTokenType>) {
        self.token.write(token);
    }

    /// Fetches an oauth token
    async fn fetch_oauth_token(&self) -> eyre::Result<Token> {
        let token_result = self
            .client
            .exchange_client_credentials()
            .request_async(async_http_client)
            .await?;

        Ok(token_result)
    }

    /// Spawns a task that periodically fetches a new token every 300 seconds.
    fn spawn(self) -> JoinHandle<()> {
        let interval = self.config.oauth_token_refresh_interval;

        let handle: JoinHandle<()> = tokio::spawn(async move {
            loop {
                info!("Refreshing oauth token");
                match self.authenticate().await {
                    Ok(_) => {
                        info!("Successfully refreshed oauth token");
                    }
                    Err(e) => {
                        error!(%e, "Failed to refresh oauth token");
                    }
                };
                let _sleep = tokio::time::sleep(tokio::time::Duration::from_secs(interval)).await;
            }
        });

        handle
    }
}

/// A shared token that can be read and written to by multiple threads.
#[derive(Debug, Clone, Default)]
pub struct SharedToken {
    inner: Arc<Mutex<Option<Token>>>,
}

impl SharedToken {
    /// Read the token from the shared token. This is `None` if the token has
    /// not yet been set.
    pub fn read(&self) -> Option<Token> {
        self.inner.lock().unwrap().clone()
    }

    /// Write a new token to the shared token.
    pub fn write(&self, token: Token) {
        let mut lock = self.inner.lock().unwrap();
        *lock = Some(token);
    }

    /// Check if the token is authenticated.
    pub fn is_authenticated(&self) -> bool {
        self.inner.lock().unwrap().is_some()
    }
}
