use crate::utils::from_env::{FromEnvErr, FromEnvVar};
use alloy::{
    providers::{IpcConnect, RootProvider, WsConnect},
    pubsub::{ConnectionHandle, PubSubConnect},
    rpc::client::BuiltInConnectionString,
    transports::{
        BoxTransport, TransportConnect, TransportError, TransportErrorKind, TransportResult,
    },
};

impl FromEnvVar for BuiltInConnectionString {
    type Error = TransportError;

    fn from_env_var(env_var: &str) -> Result<Self, FromEnvErr<Self::Error>> {
        let conn_str = String::from_env_var(env_var).map_err(FromEnvErr::infallible_into)?;
        conn_str.parse().map_err(Into::into)
    }
}

/// Configuration for an Alloy provider, sourced from an environment variable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderConfig {
    connection_string: BuiltInConnectionString,
}

impl ProviderConfig {
    /// Creates a new `ProviderConfig` from a connection string.
    pub const fn new(connection_string: BuiltInConnectionString) -> Self {
        Self { connection_string }
    }

    /// Returns the connection string.
    pub const fn connection_string(&self) -> &BuiltInConnectionString {
        &self.connection_string
    }

    /// Connects to the provider using the connection string.
    pub async fn connect(&self) -> TransportResult<RootProvider> {
        RootProvider::connect_with(self.clone()).await
    }
}

impl FromEnvVar for ProviderConfig {
    type Error = TransportError;

    fn from_env_var(env_var: &str) -> Result<Self, FromEnvErr<Self::Error>> {
        let connection_string = BuiltInConnectionString::from_env_var(env_var)?;
        Ok(Self { connection_string })
    }
}

impl TransportConnect for ProviderConfig {
    fn is_local(&self) -> bool {
        self.connection_string.is_local()
    }

    fn get_transport(
        &self,
    ) -> alloy::transports::impl_future!(<Output = Result<BoxTransport, TransportError>>) {
        self.connection_string.get_transport()
    }
}

/// Configuration for an Alloy provider, used to create a client, enforces
/// pubsub availability (WS or IPC connection).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PubSubConfig {
    connection_string: BuiltInConnectionString,
}

impl PubSubConfig {
    /// Returns the connection string.
    pub const fn connection_string(&self) -> &BuiltInConnectionString {
        &self.connection_string
    }

    /// Connects to the provider using the connection string.
    pub async fn connect(&self) -> TransportResult<RootProvider> {
        RootProvider::connect_with(self.clone()).await
    }
}

impl TryFrom<BuiltInConnectionString> for PubSubConfig {
    type Error = TransportError;

    fn try_from(connection_string: BuiltInConnectionString) -> Result<Self, Self::Error> {
        if !matches!(
            connection_string,
            BuiltInConnectionString::Ws(_, _) | BuiltInConnectionString::Ipc(_)
        ) {
            return Err(TransportErrorKind::pubsub_unavailable());
        }
        Ok(Self { connection_string })
    }
}

impl FromEnvVar for PubSubConfig {
    type Error = TransportError;

    fn from_env_var(env_var: &str) -> Result<Self, FromEnvErr<Self::Error>> {
        let cs = BuiltInConnectionString::from_env_var(env_var)?;
        Self::try_from(cs).map_err(FromEnvErr::ParseError)
    }
}

impl TransportConnect for PubSubConfig {
    fn is_local(&self) -> bool {
        self.connection_string.is_local()
    }

    fn get_transport(
        &self,
    ) -> alloy::transports::impl_future!(<Output = Result<BoxTransport, TransportError>>) {
        self.connection_string.get_transport()
    }
}

impl PubSubConnect for PubSubConfig {
    fn is_local(&self) -> bool {
        self.connection_string.is_local()
    }

    fn connect(
        &self,
    ) -> alloy::transports::impl_future!(<Output = TransportResult<ConnectionHandle>>) {
        async move {
            match &self.connection_string {
                BuiltInConnectionString::Ws(ws, auth) => {
                    WsConnect::new(ws.as_str())
                        .with_auth_opt(auth.clone())
                        .connect()
                        .await
                }
                BuiltInConnectionString::Ipc(ipc) => IpcConnect::new(ipc.clone()).connect().await,
                _ => unreachable!("can't instantiate http variant"),
            }
        }
    }
}
