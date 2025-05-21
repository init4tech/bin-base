use crate::utils::from_env::{EnvItemInfo, FromEnv, FromEnvErr, FromEnvVar};
use alloy::primitives::{hex::FromHexError, Address};
use signet_constants::{
    HostConstants, ParseChainError, PredeployTokens, RollupConstants, SignetConstants,
    SignetEnvironmentConstants, SignetSystemConstants,
};
use std::{borrow::Cow, num::ParseIntError};

/// EnvItemInfo for .env variable holding chain name
/// Used to implement FromEnv for SignetConstants type structs
/// that can be instantiated from a single chain name
const CHAIN_NAME: EnvItemInfo = EnvItemInfo {
    var: "CHAIN_NAME",
    description: "The name of the chain, e.g. `pecorino`. If CHAIN_NAME is present, the known, hard-coded constants for the chain will be loaded from the SDK. If CHAIN_NAME is not present, each constant will be loaded from environment variables.",
    optional: true,
};

// --- RollupConstants ---
const ROLLUP_CHAIN_ID: &str = "ROLLUP_CHAIN_ID";
const ROLLUP_BASE_FEE_RECIPIENT: &str = "ROLLUP_BASE_FEE_RECIPIENT";
const ROLLUP_ORDERS: &str = "ROLLUP_ORDERS";
const ROLLUP_PASSAGE: &str = "ROLLUP_PASSAGE";
const ROLLUP_USDC: &str = "ROLLUP_USDC";
const ROLLUP_USDT: &str = "ROLLUP_USDT";
const ROLLUP_WBTC: &str = "ROLLUP_WBTC";
// --- HostConstants ---
const HOST_CHAIN_ID: &str = "HOST_CHAIN_ID";
const HOST_DEPLOY_HEIGHT: &str = "HOST_DEPLOY_HEIGHT";
const HOST_ZENITH: &str = "HOST_ZENITH";
const HOST_ORDERS: &str = "HOST_ORDERS";
const HOST_PASSAGE: &str = "HOST_PASSAGE";
const HOST_TRANSACTOR: &str = "HOST_TRANSACTOR";
const HOST_USDC: &str = "HOST_USDC";
const HOST_USDT: &str = "HOST_USDT";
const HOST_WBTC: &str = "HOST_WBTC";
// --- SignetEnvironmentConstants ---
const SIGNET_HOST_NAME: &str = "SIGNET_HOST_NAME";
const SIGNET_ROLLUP_NAME: &str = "SIGNET_ROLLUP_NAME";
const SIGNET_TRANSACTION_CACHE: &str = "SIGNET_TRANSACTION_CACHE";

/// Error type for parsing SignetConstants from environment variables.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ConstantsFromEnvError {
    /// Error parsing a chain name.
    #[error(transparent)]
    ParseChainError(#[from] ParseChainError),
    /// Error parsing a u64.
    #[error(transparent)]
    ParseIntError(#[from] ParseIntError),
    /// Error parsing a hex string.
    #[error(transparent)]
    FromHexError(#[from] FromHexError),
}

impl From<FromEnvErr<ParseChainError>> for FromEnvErr<ConstantsFromEnvError> {
    fn from(e: FromEnvErr<ParseChainError>) -> Self {
        match e {
            FromEnvErr::ParseError(i) => FromEnvErr::ParseError(i.into()),
            FromEnvErr::Empty(i) => FromEnvErr::Empty(i),
            FromEnvErr::EnvError(var, err) => FromEnvErr::EnvError(var, err),
        }
    }
}

impl From<FromEnvErr<ParseIntError>> for FromEnvErr<ConstantsFromEnvError> {
    fn from(e: FromEnvErr<ParseIntError>) -> Self {
        match e {
            FromEnvErr::ParseError(i) => FromEnvErr::ParseError(i.into()),
            FromEnvErr::Empty(i) => FromEnvErr::Empty(i),
            FromEnvErr::EnvError(var, err) => FromEnvErr::EnvError(var, err),
        }
    }
}

impl From<FromEnvErr<FromHexError>> for FromEnvErr<ConstantsFromEnvError> {
    fn from(e: FromEnvErr<FromHexError>) -> Self {
        match e {
            FromEnvErr::ParseError(i) => FromEnvErr::ParseError(i.into()),
            FromEnvErr::Empty(i) => FromEnvErr::Empty(i),
            FromEnvErr::EnvError(var, err) => FromEnvErr::EnvError(var, err),
        }
    }
}

impl FromEnv for RollupConstants {
    type Error = ConstantsFromEnvError;

    fn inventory() -> Vec<&'static EnvItemInfo> {
        vec![
            &CHAIN_NAME,
            &EnvItemInfo {
                var: ROLLUP_CHAIN_ID,
                description: "Rollup chain ID.",
                optional: false,
            },
            &EnvItemInfo {
                var: ROLLUP_BASE_FEE_RECIPIENT,
                description: "Rollup address of the base fee recipient.",
                optional: false,
            },
            &EnvItemInfo {
                var: ROLLUP_ORDERS,
                description: "Rollup address of the orders contract.",
                optional: false,
            },
            &EnvItemInfo {
                var: ROLLUP_PASSAGE,
                description: "Rollup address of the passage contract.",
                optional: false,
            },
            &EnvItemInfo {
                var: ROLLUP_USDC,
                description: "Rollup address of usdc token.",
                optional: false,
            },
            &EnvItemInfo {
                var: ROLLUP_USDT,
                description: "Rollup address of usdt token.",
                optional: false,
            },
            &EnvItemInfo {
                var: ROLLUP_WBTC,
                description: "Rollup address of wbtc token.",
                optional: false,
            },
        ]
    }

    fn from_env() -> Result<Self, FromEnvErr<Self::Error>> {
        match Self::from_env_var(CHAIN_NAME.var) {
            Ok(c) => Ok(c),
            Err(e) => {
                match e {
                    // if chain name is present but malformed, propagate the error
                    FromEnvErr::ParseError(_) => Err(e.into()),
                    // if the chain name is empty or missing,
                    // instantiate each prop from env vars
                    FromEnvErr::EnvError(_, _) | FromEnvErr::Empty(_) => Ok(RollupConstants::new(
                        u64::from_env_var(ROLLUP_CHAIN_ID)?,
                        Address::from_env_var(ROLLUP_ORDERS)?,
                        Address::from_env_var(ROLLUP_PASSAGE)?,
                        Address::from_env_var(ROLLUP_BASE_FEE_RECIPIENT)?,
                        PredeployTokens::new(
                            Address::from_env_var(ROLLUP_USDC)?,
                            Address::from_env_var(ROLLUP_USDT)?,
                            Address::from_env_var(ROLLUP_WBTC)?,
                        ),
                    )),
                }
            }
        }
    }
}

impl FromEnv for HostConstants {
    type Error = ConstantsFromEnvError;

    fn inventory() -> Vec<&'static EnvItemInfo> {
        vec![
            &CHAIN_NAME,
            &EnvItemInfo {
                var: HOST_CHAIN_ID,
                description: "Host chain ID.",
                optional: false,
            },
            &EnvItemInfo {
                var: HOST_DEPLOY_HEIGHT,
                description: "Height at which the host chain deployed the rollup contracts.",
                optional: false,
            },
            &EnvItemInfo {
                var: HOST_ZENITH,
                description: "Host address for the zenith contract",
                optional: false,
            },
            &EnvItemInfo {
                var: HOST_ORDERS,
                description: "Host address for the orders contract",
                optional: false,
            },
            &EnvItemInfo {
                var: HOST_PASSAGE,
                description: "Host address for the passage contract",
                optional: false,
            },
            &EnvItemInfo {
                var: HOST_TRANSACTOR,
                description: "Host address for the transactor contract",
                optional: false,
            },
            &EnvItemInfo {
                var: HOST_USDC,
                description: "Host address for the USDC token",
                optional: false,
            },
            &EnvItemInfo {
                var: HOST_USDT,
                description: "Host address for the USDT token",
                optional: false,
            },
            &EnvItemInfo {
                var: HOST_WBTC,
                description: "Host address for the WBTC token",
                optional: false,
            },
        ]
    }

    fn from_env() -> Result<Self, FromEnvErr<Self::Error>> {
        match Self::from_env_var(CHAIN_NAME.var) {
            Ok(c) => Ok(c),
            Err(e) => {
                match e {
                    // if chain name is present but malformed, propagate the error
                    FromEnvErr::ParseError(_) => Err(e.into()),
                    // if the chain name is empty or missing,
                    // instantiate each prop from env vars
                    FromEnvErr::EnvError(_, _) | FromEnvErr::Empty(_) => Ok(HostConstants::new(
                        u64::from_env_var(HOST_CHAIN_ID)?,
                        u64::from_env_var(HOST_DEPLOY_HEIGHT)?,
                        Address::from_env_var(HOST_ZENITH)?,
                        Address::from_env_var(HOST_ORDERS)?,
                        Address::from_env_var(HOST_PASSAGE)?,
                        Address::from_env_var(HOST_TRANSACTOR)?,
                        PredeployTokens::new(
                            Address::from_env_var(HOST_USDC)?,
                            Address::from_env_var(HOST_USDT)?,
                            Address::from_env_var(HOST_WBTC)?,
                        ),
                    )),
                }
            }
        }
    }
}

impl FromEnv for SignetEnvironmentConstants {
    type Error = ConstantsFromEnvError;

    fn inventory() -> Vec<&'static EnvItemInfo> {
        vec![
            &CHAIN_NAME,
            &EnvItemInfo {
                var: SIGNET_HOST_NAME,
                description: "Name of the host chain.",
                optional: false,
            },
            &EnvItemInfo {
                var: SIGNET_ROLLUP_NAME,
                description: "Name of the rollup.",
                optional: false,
            },
            &EnvItemInfo {
                var: SIGNET_TRANSACTION_CACHE,
                description: "URL of the Transaction Cache",
                optional: false,
            },
        ]
    }

    fn from_env() -> Result<Self, FromEnvErr<Self::Error>> {
        match Self::from_env_var(CHAIN_NAME.var) {
            Ok(c) => Ok(c),
            Err(e) => {
                match e {
                    // if chain name is present but malformed, propagate the error
                    FromEnvErr::ParseError(_) => Err(e.into()),
                    // if the chain name is empty or missing,
                    // instantiate each prop from env vars
                    FromEnvErr::EnvError(_, _) | FromEnvErr::Empty(_) => {
                        Ok(SignetEnvironmentConstants::new(
                            Cow::from_env_var(SIGNET_HOST_NAME)
                                .map_err(|e| e.infallible_into::<ConstantsFromEnvError>())?,
                            Cow::from_env_var(SIGNET_ROLLUP_NAME)
                                .map_err(|e| e.infallible_into::<ConstantsFromEnvError>())?,
                            Cow::from_env_var(SIGNET_TRANSACTION_CACHE)
                                .map_err(|e| e.infallible_into::<ConstantsFromEnvError>())?,
                        ))
                    }
                }
            }
        }
    }
}

impl FromEnv for SignetSystemConstants {
    type Error = ConstantsFromEnvError;

    fn inventory() -> Vec<&'static EnvItemInfo> {
        let mut inventory = Vec::new();
        inventory.extend_from_slice(&HostConstants::inventory());
        inventory.extend_from_slice(&RollupConstants::inventory()[1..]);
        inventory
    }

    fn from_env() -> Result<Self, FromEnvErr<Self::Error>> {
        match Self::from_env_var(CHAIN_NAME.var) {
            Ok(c) => Ok(c),
            Err(e) => {
                match e {
                    // if chain name is present but malformed, propagate the error
                    FromEnvErr::ParseError(_) => Err(e.into()),
                    // if the chain name is empty or missing,
                    // instantiate each prop from env vars
                    FromEnvErr::EnvError(_, _) | FromEnvErr::Empty(_) => {
                        Ok(SignetSystemConstants::new(
                            HostConstants::from_env()?,
                            RollupConstants::from_env()?,
                        ))
                    }
                }
            }
        }
    }
}

impl FromEnv for SignetConstants {
    type Error = ConstantsFromEnvError;

    fn inventory() -> Vec<&'static EnvItemInfo> {
        let mut inventory = Vec::new();
        inventory.extend_from_slice(&SignetSystemConstants::inventory());
        inventory.extend_from_slice(&SignetEnvironmentConstants::inventory()[1..]);
        inventory
    }

    fn from_env() -> Result<Self, FromEnvErr<Self::Error>> {
        match Self::from_env_var(CHAIN_NAME.var) {
            Ok(c) => Ok(c),
            Err(e) => {
                match e {
                    // if chain name is present but malformed, propagate the error
                    FromEnvErr::ParseError(_) => Err(e.into()),
                    // if the chain name is empty or missing,
                    // instantiate each prop from env vars
                    FromEnvErr::EnvError(_, _) | FromEnvErr::Empty(_) => Ok(SignetConstants::new(
                        SignetSystemConstants::from_env()?,
                        SignetEnvironmentConstants::from_env()?,
                    )),
                }
            }
        }
    }
}
