use crate::utils::from_env::FromEnv;
use alloy::{
    consensus::SignableTransaction,
    network::{Ethereum, EthereumWallet, IntoWallet},
    primitives::{Address, ChainId, B256},
    signers::{
        aws::{AwsSigner, AwsSignerError},
        local::{LocalSignerError, PrivateKeySigner},
        Signature,
    },
};
use aws_config::{load_defaults, BehaviorVersion};
use aws_sdk_kms::Client;
use std::borrow::Cow;

/// Configuration for a LocalOrAws signer.
///
/// Usage:
/// ```
/// # async fn test() -> Result<(), Box<dyn std::error::Error>> {
/// use init4_bin_base::utils::{signer::LocalOrAwsConfig, from_env::FromEnv};
/// let signer = LocalOrAwsConfig::from_env()?.connect().await?;
/// # Ok(())
/// # }
/// ```
#[derive(FromEnv, Debug, Clone)]
#[from_env(crate)]
pub struct LocalOrAwsConfig {
    /// The private key or AWS signer key ID.
    #[from_env(var = "SIGNER_KEY", desc = "AWS KMS key ID or local private key")]
    key_info: Cow<'static, str>,
    /// Chain ID for the AWS signer.
    #[from_env(var = "SIGNER_CHAIN_ID", desc = "Chain ID for AWS signer", optional)]
    chain_id: Option<u64>,
}

impl LocalOrAwsConfig {
    /// Connect signer, but only if remote
    pub async fn connect_remote(&self) -> Result<LocalOrAws, SignerError> {
        let signer = LocalOrAws::aws_signer(&self.key_info, self.chain_id).await?;
        Ok(LocalOrAws::Aws(signer))
    }

    /// Connect signer, but only if local
    pub fn connect_local(&self) -> Result<LocalOrAws, SignerError> {
        Ok(LocalOrAws::Local(LocalOrAws::wallet(&self.key_info)?))
    }

    /// Connect signer, either local or remote
    pub async fn connect(&self) -> Result<LocalOrAws, SignerError> {
        if let Ok(local) = self.connect_local() {
            Ok(local)
        } else {
            self.connect_remote().await
        }
    }
}

/// Abstraction over local signer or
#[derive(Debug, Clone)]
pub enum LocalOrAws {
    /// Local signer
    Local(PrivateKeySigner),
    /// AWS signer
    Aws(AwsSigner),
}

/// Error during signing
#[derive(Debug, thiserror::Error)]
pub enum SignerError {
    /// Error during [`AwsSigner`] instantiation
    #[error("failed to connect AWS signer: {0}")]
    AwsSigner(#[from] Box<AwsSignerError>),
    /// Error loading the private key
    #[error("failed to load private key: {0}")]
    Wallet(#[from] LocalSignerError),
    /// Error parsing hex
    #[error("failed to parse hex: {0}")]
    Hex(#[from] alloy::hex::FromHexError),
}

impl From<AwsSignerError> for SignerError {
    fn from(err: AwsSignerError) -> Self {
        SignerError::AwsSigner(Box::new(err))
    }
}

impl LocalOrAws {
    /// Load a privkey or AWS signer from environment variables.
    pub async fn load(key: &str, chain_id: Option<u64>) -> Result<Self, SignerError> {
        if let Ok(wallet) = LocalOrAws::wallet(key) {
            Ok(LocalOrAws::Local(wallet))
        } else {
            let signer = LocalOrAws::aws_signer(key, chain_id).await?;
            Ok(LocalOrAws::Aws(signer))
        }
    }

    /// Load the wallet from environment variables.
    ///
    /// # Panics
    ///
    /// Panics if the env var contents is not a valid secp256k1 private key.
    fn wallet(private_key: &str) -> Result<PrivateKeySigner, SignerError> {
        let bytes = alloy::hex::decode(private_key.strip_prefix("0x").unwrap_or(private_key))?;
        Ok(PrivateKeySigner::from_slice(&bytes).unwrap())
    }

    /// Load the AWS signer from environment variables./s
    async fn aws_signer(key_id: &str, chain_id: Option<u64>) -> Result<AwsSigner, SignerError> {
        let config = load_defaults(BehaviorVersion::latest()).await;
        let client = Client::new(&config);
        AwsSigner::new(client, key_id.to_string(), chain_id)
            .await
            .map_err(Into::into)
    }
}

#[async_trait::async_trait]
impl alloy::network::TxSigner<Signature> for LocalOrAws {
    fn address(&self) -> Address {
        match self {
            LocalOrAws::Local(signer) => signer.address(),
            LocalOrAws::Aws(signer) => signer.address(),
        }
    }

    async fn sign_transaction(
        &self,
        tx: &mut dyn SignableTransaction<Signature>,
    ) -> alloy::signers::Result<Signature> {
        match self {
            LocalOrAws::Local(signer) => signer.sign_transaction(tx).await,
            LocalOrAws::Aws(signer) => signer.sign_transaction(tx).await,
        }
    }
}

#[async_trait::async_trait]
impl alloy::signers::Signer<Signature> for LocalOrAws {
    /// Signs the given hash.
    async fn sign_hash(&self, hash: &B256) -> alloy::signers::Result<Signature> {
        match self {
            LocalOrAws::Local(signer) => signer.sign_hash(hash).await,
            LocalOrAws::Aws(signer) => signer.sign_hash(hash).await,
        }
    }

    /// Returns the signer's Ethereum Address.
    fn address(&self) -> Address {
        match self {
            LocalOrAws::Local(signer) => signer.address(),
            LocalOrAws::Aws(signer) => signer.address(),
        }
    }

    /// Returns the signer's chain ID.
    fn chain_id(&self) -> Option<ChainId> {
        match self {
            LocalOrAws::Local(signer) => signer.chain_id(),
            LocalOrAws::Aws(signer) => signer.chain_id(),
        }
    }

    /// Sets the signer's chain ID.
    fn set_chain_id(&mut self, chain_id: Option<ChainId>) {
        match self {
            LocalOrAws::Local(signer) => signer.set_chain_id(chain_id),
            LocalOrAws::Aws(signer) => signer.set_chain_id(chain_id),
        }
    }
}

impl IntoWallet<Ethereum> for LocalOrAws {
    type NetworkWallet = EthereumWallet;

    fn into_wallet(self) -> Self::NetworkWallet {
        EthereumWallet::from(self)
    }
}
