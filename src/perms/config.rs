use crate::utils::from_env::{FromEnv, FromEnvErr, FromEnvVar};
use core::num;

// Environment variable names for configuration
const CHAIN_OFFSET: &str = "CHAIN_OFFSET";
const BLOCK_QUERY_CUTOFF: &str = "BLOCK_QUERY_CUTOFF";
const BLOCK_QUERY_START: &str = "BLOCK_QUERY_START";

/// Possible errors when loading the slot authorization configuration.
#[derive(thiserror::Error, Debug, Clone, PartialEq, Eq)]
pub enum SlotAuthzConfigError {
    /// Error reading environment variable.
    #[error("error reading chain offset: {0}")]
    ChainOffset(num::ParseIntError),
    /// Error reading block query cutoff.
    #[error("error reading block query cutoff: {0}")]
    BlockQueryCutoff(num::ParseIntError),
    /// Error reading block query start.
    #[error("error reading block query start: {0}")]
    BlockQueryStart(num::ParseIntError),
}

/// Configuration object that describes the slot time settings for a chain.
///
/// This struct is used to configure the slot authorization system
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SlotAuthzConfig {
    /// The chain offset in seconds. The offset is the a block's timestamp %
    /// its slot duration. This is used to calculate the slot number for a
    /// given unix epoch timestamp.
    ///
    /// On loading from env, the number will be clamped between 0 and 11, as
    /// the slot duration is 12 seconds.
    chain_offset: u8,
    /// The block query cutoff time in seconds. This is the slot second after
    /// which requests will not be serviced. E.g. a value of 1 means that
    /// requests will not be serviced for the last second of any given slot.
    ///
    /// On loading from env, the number will be clamped between 0 and 11, as
    /// the slot duration is 12 seconds.
    block_query_cutoff: u8,
    /// The block query start time in seconds. This is the slot second before
    /// which requests will not be serviced. E.g. a value of 1 means that
    /// requests will not be serviced for the first second of any given slot.
    ///
    /// On loading from env, the number will be clamped between 0 and 11, as
    /// the slot duration is 12 seconds.
    block_query_start: u8,
}

impl SlotAuthzConfig {
    /// Creates a new `SlotAuthzConfig` with the given parameters, clamping the
    /// values between 0 and 11.
    pub fn new(chain_offset: u8, block_query_cutoff: u8, block_query_start: u8) -> Self {
        Self {
            chain_offset: chain_offset.clamp(0, 11),
            block_query_cutoff: block_query_cutoff.clamp(0, 11),
            block_query_start: block_query_start.clamp(0, 11),
        }
    }

    /// Get the chain offset in seconds.
    pub fn chain_offset(&self) -> u64 {
        self.chain_offset as u64
    }

    /// Get the block query cutoff time in seconds.
    pub fn block_query_cutoff(&self) -> u64 {
        self.block_query_cutoff as u64
    }

    /// Get the block query start time in seconds.
    pub fn block_query_start(&self) -> u64 {
        self.block_query_start as u64
    }
}

impl FromEnv for SlotAuthzConfig {
    type Error = SlotAuthzConfigError;

    fn from_env() -> Result<Self, FromEnvErr<Self::Error>> {
        let chain_offset = u8::from_env_var(CHAIN_OFFSET)
            .map_err(|e| e.map(SlotAuthzConfigError::ChainOffset))?
            .clamp(0, 11);
        let block_query_cutoff = u8::from_env_var(BLOCK_QUERY_CUTOFF)
            .map_err(|e| e.map(SlotAuthzConfigError::BlockQueryCutoff))?
            .clamp(0, 11);
        let block_query_start = u8::from_env_var(BLOCK_QUERY_START)
            .map_err(|e| e.map(SlotAuthzConfigError::BlockQueryStart))?
            .clamp(0, 11);

        Ok(Self {
            chain_offset,
            block_query_cutoff,
            block_query_start,
        })
    }
}
