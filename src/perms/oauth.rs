//! Service responsible for authenticating with the cache with Oauth tokens.
//! This authenticator periodically fetches a new token every set amount of seconds.
use crate::{
    deps::tracing::{error, info},
    utils::from_env::FromEnv,
};
use oauth2::{
    basic::{BasicClient, BasicTokenType},
    AuthUrl, ClientId, ClientSecret, EmptyExtraTokenFields, EndpointNotSet, EndpointSet,
    HttpClientError, RequestTokenError, StandardErrorResponse, StandardTokenResponse, TokenUrl,
};
use std::sync::{Arc, Mutex};
use tokio::task::JoinHandle;

type Token = StandardTokenResponse<EmptyExtraTokenFields, BasicTokenType>;

type MyOAuthClient =
    BasicClient<EndpointSet, EndpointNotSet, EndpointNotSet, EndpointNotSet, EndpointSet>;

/// Configuration for the OAuth2 client.
#[derive(Debug, Clone, FromEnv)]
#[from_env(crate)]
pub struct OAuthConfig {
    /// OAuth client ID for the builder.
    #[from_env(var = "OAUTH_CLIENT_ID", desc = "OAuth client ID for the builder")]
    pub oauth_client_id: String,
    /// OAuth client secret for the builder.
    #[from_env(
        var = "OAUTH_CLIENT_SECRET",
        desc = "OAuth client secret for the builder"
    )]
    pub oauth_client_secret: String,
    /// OAuth authenticate URL for the builder for performing OAuth logins.
    #[from_env(
        var = "OAUTH_AUTHENTICATE_URL",
        desc = "OAuth authenticate URL for the builder for performing OAuth logins"
    )]
    pub oauth_authenticate_url: url::Url,
    /// OAuth token URL for the builder to get an OAuth2 access token
    #[from_env(
        var = "OAUTH_TOKEN_URL",
        desc = "OAuth token URL for the builder to get an OAuth2 access token"
    )]
    pub oauth_token_url: url::Url,
    /// The oauth token refresh interval in seconds.
    #[from_env(
        var = "AUTH_TOKEN_REFRESH_INTERVAL",
        desc = "The oauth token refresh interval in seconds"
    )]
    pub oauth_token_refresh_interval: u64,
}

impl OAuthConfig {
    /// Create a new [`Authenticator`] from the provided config.
    pub fn authenticator(&self) -> Authenticator {
        Authenticator::new(self)
    }
}

/// A shared token that can be read and written to by multiple threads.
#[derive(Debug, Clone, Default)]
pub struct SharedToken(Arc<Mutex<Option<Token>>>);

impl SharedToken {
    /// Read the token from the shared token.
    pub fn read(&self) -> Option<Token> {
        self.0.lock().unwrap().clone()
    }

    /// Write a new token to the shared token.
    pub fn write(&self, token: Token) {
        let mut lock = self.0.lock().unwrap();
        *lock = Some(token);
    }

    /// Check if the token is authenticated.
    pub fn is_authenticated(&self) -> bool {
        self.0.lock().unwrap().is_some()
    }
}

/// A self-refreshing, periodically fetching authenticator for the block
/// builder. This task periodically fetches a new token, and stores it in a
/// [`SharedToken`].
#[derive(Debug)]
pub struct Authenticator {
    /// Configuration
    pub config: OAuthConfig,
    client: MyOAuthClient,
    token: SharedToken,
    reqwest: reqwest::Client,
}

impl Authenticator {
    /// Creates a new Authenticator from the provided builder config.
    pub fn new(config: &OAuthConfig) -> Self {
        let client = BasicClient::new(ClientId::new(config.oauth_client_id.clone()))
            .set_client_secret(ClientSecret::new(config.oauth_client_secret.clone()))
            .set_auth_uri(AuthUrl::from_url(config.oauth_authenticate_url.clone()))
            .set_token_uri(TokenUrl::from_url(config.oauth_token_url.clone()));

        let rq_client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .unwrap();

        Self {
            config: config.clone(),
            client,
            token: Default::default(),
            reqwest: rq_client,
        }
    }

    /// Requests a new authentication token and, if successful, sets it to as the token
    pub async fn authenticate(
        &self,
    ) -> Result<
        (),
        RequestTokenError<
            HttpClientError<reqwest::Error>,
            StandardErrorResponse<oauth2::basic::BasicErrorResponseType>,
        >,
    > {
        let token = self.fetch_oauth_token().await?;
        self.set_token(token);
        Ok(())
    }

    /// Returns true if there is Some token set
    pub fn is_authenticated(&self) -> bool {
        self.token.is_authenticated()
    }

    /// Sets the Authenticator's token to the provided value
    fn set_token(&self, token: StandardTokenResponse<EmptyExtraTokenFields, BasicTokenType>) {
        self.token.write(token);
    }

    /// Returns the currently set token
    pub fn token(&self) -> SharedToken {
        self.token.clone()
    }

    /// Fetches an oauth token
    pub async fn fetch_oauth_token(
        &self,
    ) -> Result<
        Token,
        RequestTokenError<
            HttpClientError<reqwest::Error>,
            StandardErrorResponse<oauth2::basic::BasicErrorResponseType>,
        >,
    > {
        let token_result = self
            .client
            .exchange_client_credentials()
            .request_async(&self.reqwest)
            .await?;

        Ok(token_result)
    }

    /// Spawns a task that periodically fetches a new token every 300 seconds.
    pub fn spawn(self) -> JoinHandle<()> {
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
