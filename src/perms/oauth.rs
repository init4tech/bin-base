//! Service responsible for authenticating with the cache with Oauth tokens.
//! This authenticator periodically fetches a new token every set amount of seconds.
use crate::{deps::tracing::error, utils::from_env::FromEnv};
use core::{error::Error, fmt};
use oauth2::{
    basic::{BasicClient, BasicTokenType},
    AccessToken, AuthUrl, ClientId, ClientSecret, EmptyExtraTokenFields, EndpointNotSet,
    EndpointSet, HttpClientError, RefreshToken, RequestTokenError, Scope, StandardErrorResponse,
    StandardTokenResponse, TokenResponse, TokenUrl,
};
use std::{future::IntoFuture, pin::Pin};
use tokio::{
    sync::watch::{self, Ref},
    task::JoinHandle,
};
use tracing::{debug, Instrument};

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

/// A self-refreshing, periodically fetching authenticator for the block
/// builder. This task periodically fetches a new token, and sends it to all
/// active [`SharedToken`]s via a [`tokio::sync::watch`] channel.
///
/// This task can be spawned using the [`Authenticator::spawn`] method, which
/// will create a new tokio task that runs the refresh loop in the background,
/// in the current [`tracing`] span. Alternately, the [`IntoFuture`]
/// implementation can be used to create a future that runs the refresh loop,
/// and can be isntrumented with the [`Instrument`] trait, and then spawned or
/// awaited as desired.
#[derive(Debug)]
pub struct Authenticator {
    /// Configuration
    config: OAuthConfig,
    client: MyOAuthClient,
    reqwest: reqwest::Client,

    token: watch::Sender<Option<Token>>,
}

impl Authenticator {
    /// Creates a new Authenticator from the provided builder config.
    pub fn new(config: &OAuthConfig) -> Self {
        let client = BasicClient::new(ClientId::new(config.oauth_client_id.clone()))
            .set_client_secret(ClientSecret::new(config.oauth_client_secret.clone()))
            .set_auth_uri(AuthUrl::from_url(config.oauth_authenticate_url.clone()))
            .set_token_uri(TokenUrl::from_url(config.oauth_token_url.clone()));

        // NB: this is MANDATORY
        // https://docs.rs/oauth2/latest/oauth2/#security-warning
        let rq_client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .unwrap();

        Self {
            config: config.clone(),
            client,
            reqwest: rq_client,
            token: watch::channel(None).0,
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
        self.token.borrow().is_some()
    }

    /// Sets the Authenticator's token to the provided value
    fn set_token(&self, token: StandardTokenResponse<EmptyExtraTokenFields, BasicTokenType>) {
        self.token.send_replace(Some(token));
    }

    /// Returns the currently set token
    pub fn token(&self) -> SharedToken {
        self.token.subscribe().into()
    }

    /// Fetches an oauth token.
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

    /// Get a reference to the OAuth configuration.
    pub const fn config(&self) -> &OAuthConfig {
        &self.config
    }

    /// Create a future that contains the periodic refresh loop.
    async fn task_future(self) {
        let interval = self.config.oauth_token_refresh_interval;

        loop {
            debug!("Refreshing oauth token");
            match self.authenticate().await {
                Ok(_) => {
                    debug!("Successfully refreshed oauth token");
                }
                Err(err) => {
                    let mut current = &err as &dyn Error;

                    // This is a little hacky, but the oauth library nests
                    // errors quite deeply, so we need to walk the source chain
                    // to get the full picture.
                    let mut source_chain = Vec::new();
                    while let Some(source) = current.source() {
                        source_chain.push(source.to_string());
                        current = source;
                    }
                    let source_chain = source_chain.join("\n\n Caused by: \n");

                    error!(%err, %source_chain, "Failed to refresh oauth token");
                }
            };
            let _sleep = tokio::time::sleep(tokio::time::Duration::from_secs(interval)).await;
        }
    }

    /// Spawns a task that periodically fetches a new token. The refresh
    /// interval may be configured via the
    /// [`OAuthConfig::oauth_token_refresh_interval`] property.
    pub fn spawn(self) -> JoinHandle<()> {
        tokio::spawn(self.task_future().in_current_span())
    }
}

impl IntoFuture for Authenticator {
    type Output = ();

    type IntoFuture = Pin<Box<dyn std::future::Future<Output = ()> + Send>>;

    fn into_future(self) -> Self::IntoFuture {
        Box::pin(self.task_future())
    }
}

/// A shared token, wrapped in a [`tokio::sync::watch`] Receiver. The token is
/// periodically refreshed by an [`Authenticator`] task, and can be awaited
/// for when it becomes available.
///
/// This allows multiple tasks to wait for the token to be available, and
/// provides a way to check if the token is authenticated without blocking.
/// Please consult the [`Receiver`] documentation for caveats regarding
/// usage.
///
/// [`Receiver`]: tokio::sync::watch::Receiver
#[derive(Debug, Clone)]
pub struct SharedToken(watch::Receiver<Option<Token>>);

impl From<watch::Receiver<Option<Token>>> for SharedToken {
    fn from(inner: watch::Receiver<Option<Token>>) -> Self {
        Self(inner)
    }
}

impl SharedToken {
    /// Wait for the token to be available, and get a reference to the secret.
    ///
    /// This is implemented using [`Receiver::wait_for`], and has the same
    /// blocking, panics, errors, and cancel safety. However, it uses a clone
    /// of the [`watch::Receiver`] and will not update the local view of the
    /// channel.
    ///
    /// [`Receiver::wait_for`]: tokio::sync::watch::Receiver::wait_for
    pub async fn secret(&self) -> Result<String, watch::error::RecvError> {
        Ok(self
            .clone()
            .token()
            .await?
            .access_token()
            .secret()
            .to_owned())
    }

    /// Wait for the token to be available, then get a reference to it.
    ///
    /// Holding this reference will block the background task from updating
    /// the token until it is dropped, so it is recommended to drop this
    /// reference as soon as possible.
    ///
    /// This is implemented using [`Receiver::wait_for`], and has the same
    /// blocking, panics, errors, and cancel safety. Unlike [`Self::secret`]
    /// it is NOT implemented using a clone, and will update the local view of
    /// the channel.
    ///
    /// Generally, prefer using [`Self::secret`] for simple use cases, and
    /// this when deeper inspection of the token is required.
    ///
    /// [`Receiver::wait_for`]: tokio::sync::watch::Receiver::wait_for
    pub async fn token(&mut self) -> Result<TokenRef<'_>, watch::error::RecvError> {
        self.0.wait_for(Option::is_some).await.map(Into::into)
    }

    /// Create a future that will resolve when the token is ready.
    ///
    /// This is implemented using [`Receiver::wait_for`], and has the same
    /// blocking, panics, errors, and cancel safety.
    ///
    /// [`Receiver::wait_for`]: tokio::sync::watch::Receiver::wait_for
    pub async fn wait(&self) -> Result<(), watch::error::RecvError> {
        self.clone().0.wait_for(Option::is_some).await.map(drop)
    }

    /// Borrow the current token, if available. If called before the token is
    /// set by the authentication task, this will return `None`.
    ///
    /// Holding this reference will block the background task from updating
    /// the token until it is dropped, so it is recommended to drop this
    /// reference as soon as possible.
    ///
    /// This is implemented using [`Receiver::borrow`].
    ///
    /// [`Receiver::borrow`]: tokio::sync::watch::Receiver::borrow
    pub fn borrow(&mut self) -> Ref<'_, Option<Token>> {
        self.0.borrow()
    }

    /// Check if the background task has produced an authentication token.
    ///
    /// Holding this reference will block the background task from updating
    /// the token until it is dropped, so it is recommended to drop this
    /// reference as soon as possible.
    ///
    /// This is implemented using [`Receiver::borrow`].
    ///
    /// [`Receiver::borrow`]: tokio::sync::watch::Receiver::borrow
    pub fn is_authenticated(&self) -> bool {
        self.0.borrow().is_some()
    }
}

#[doc(hidden)]
impl SharedToken {
    /// Create an empty `SharedToken` that will never be authenticated.
    pub fn empty() -> Self {
        Self(watch::channel(None).1)
    }
}

/// A reference to token data, contained in a [`SharedToken`].
///
/// This is implemented using [`watch::Ref`], and as a result holds a lock on
/// the token data. Holding this reference will block the background task
/// from updating the token until it is dropped, so it is recommended to drop
/// this reference as soon as possible.
pub struct TokenRef<'a> {
    inner: Ref<'a, Option<Token>>,
}

impl<'a> From<Ref<'a, Option<Token>>> for TokenRef<'a> {
    fn from(inner: Ref<'a, Option<Token>>) -> Self {
        Self { inner }
    }
}

impl fmt::Debug for TokenRef<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TokenRef").finish_non_exhaustive()
    }
}

impl<'a> TokenRef<'a> {
    /// Get a reference to the inner token.
    pub fn inner(&'a self) -> &'a Token {
        self.inner.as_ref().unwrap()
    }

    /// Get a reference to the [`AccessToken`] contained in the token.
    pub fn access_token(&self) -> &AccessToken {
        self.inner().access_token()
    }

    /// Get a reference to the [`TokenType`] instance contained in the token.
    ///
    /// [`TokenType`]: oauth2::TokenType
    pub fn token_type(&self) -> &<Token as TokenResponse>::TokenType {
        self.inner().token_type()
    }

    /// Get a reference to the current token's expiration time, if it has one.
    pub fn expires_in(&self) -> Option<std::time::Duration> {
        self.inner().expires_in()
    }

    /// Get a reference to the refresh token, if it exists.
    pub fn refresh_token(&self) -> Option<&RefreshToken> {
        self.inner().refresh_token()
    }

    /// Get a reference to the scopes associated with the token, if any.
    pub fn scopes(&self) -> Option<&Vec<Scope>> {
        self.inner().scopes()
    }
}
