use crate::utils::from_env::{FromEnvErr, FromEnvVar};
use alloy::{
    providers::{IpcConnect, RootProvider, WsConnect},
    pubsub::{ConnectionHandle, PubSubConnect},
    rpc::client::BuiltInConnectionString,
    transports::{
        BoxTransport, TransportConnect, TransportError, TransportErrorKind, TransportResult,
    },
};

/// Errors when connecting a provider
#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq)]
pub enum ProviderConnectError {
    /// Pubsub is not available for the configured transport
    #[error("pubsub is not available for the configured transport")]
    PubsubUnavailable,
    /// Custom error message
    #[error("{0}")]
    Custom(String),
}

impl From<TransportErrorKind> for ProviderConnectError {
    fn from(err: TransportErrorKind) -> Self {
        match err {
            TransportErrorKind::Custom(err) => ProviderConnectError::Custom(err.to_string()),
            TransportErrorKind::PubsubUnavailable => ProviderConnectError::PubsubUnavailable,
            _ => panic!("Unexpected TransportErrorKind variant: {err:?}"),
        }
    }
}

impl From<TransportError> for ProviderConnectError {
    fn from(err: TransportError) -> Self {
        match err {
            TransportError::Transport(e) => e.into(),
            _ => panic!("Unexpected TransportError variant: {err:?}"),
        }
    }
}

impl FromEnvVar for BuiltInConnectionString {
    fn from_env_var(env_var: &str) -> Result<Self, FromEnvErr> {
        let conn_str = String::from_env_var(env_var).map_err(FromEnvErr::infallible_into)?;
        let built_in = conn_str
            .parse()
            .map_err(|error| FromEnvErr::parse_error(env_var, ProviderConnectError::from(error)))?;
        Ok(built_in)
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
    fn from_env_var(env_var: &str) -> Result<Self, FromEnvErr> {
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
    type Error = ProviderConnectError;

    fn try_from(connection_string: BuiltInConnectionString) -> Result<Self, Self::Error> {
        if !matches!(
            connection_string,
            BuiltInConnectionString::Ws(_, _) | BuiltInConnectionString::Ipc(_)
        ) {
            return Err(ProviderConnectError::PubsubUnavailable);
        }
        Ok(Self { connection_string })
    }
}

impl FromEnvVar for PubSubConfig {
    fn from_env_var(env_var: &str) -> Result<Self, FromEnvErr> {
        let cs = BuiltInConnectionString::from_env_var(env_var)?;
        Self::try_from(cs).map_err(|error| FromEnvErr::parse_error(env_var, error))
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

#[cfg(test)]
mod test {
    use super::*;
    use crate::utils::from_env::FromEnv;

    #[derive(FromEnv, Debug, Clone, PartialEq, Eq)]
    #[from_env(crate)]
    #[allow(dead_code)]
    struct CompileCheck {
        #[from_env(var = "COOL_DUDE", desc = "provider")]
        cool_dude: ProviderConfig,
        #[from_env(var = "COOL_DUDE2", desc = "provider2")]
        cool_dude2: PubSubConfig,
    }
}
