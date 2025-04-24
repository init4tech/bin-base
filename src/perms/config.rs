use crate::utils::{
    calc::{SlotCalcEnvError, SlotCalculator},
    from_env::{FromEnv, FromEnvErr, FromEnvVar},
};
use core::num;

// Environment variable names for configuration
pub(crate) const BLOCK_QUERY_CUTOFF: &str = "BLOCK_QUERY_CUTOFF";
pub(crate) const BLOCK_QUERY_START: &str = "BLOCK_QUERY_START";

/// Possible errors when loading the slot authorization configuration.
#[derive(thiserror::Error, Debug, Clone, PartialEq, Eq)]
pub enum SlotAuthzConfigError {
    /// Error reading environment variable.
    #[error("error reading chain offset: {0}")]
    Calculator(#[from] SlotCalcEnvError),
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
    /// A [`SlotCalculator`] instance that can be used to calculate the slot
    /// number for a given timestamp.
    calc: SlotCalculator,
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
    /// values between 0 and `calc.slot_duration()`.
    pub fn new(calc: SlotCalculator, block_query_cutoff: u8, block_query_start: u8) -> Self {
        Self {
            calc,
            block_query_cutoff: block_query_cutoff.clamp(0, calc.slot_duration() as u8),
            block_query_start: block_query_start.clamp(0, calc.slot_duration() as u8),
        }
    }

    /// Get the slot calculator instance.
    pub const fn calc(&self) -> SlotCalculator {
        self.calc
    }

    /// Get the block query cutoff time in seconds.
    pub const fn block_query_cutoff(&self) -> u64 {
        self.block_query_cutoff as u64
    }

    /// Get the block query start time in seconds.
    pub const fn block_query_start(&self) -> u64 {
        self.block_query_start as u64
    }
}

impl FromEnv for SlotAuthzConfig {
    type Error = SlotAuthzConfigError;

    fn from_env() -> Result<Self, FromEnvErr<Self::Error>> {
        let calc = SlotCalculator::from_env().map_err(FromEnvErr::from)?;
        let block_query_cutoff = u8::from_env_var(BLOCK_QUERY_CUTOFF)
            .map_err(|e| e.map(SlotAuthzConfigError::BlockQueryCutoff))?
            .clamp(0, 11);
        let block_query_start = u8::from_env_var(BLOCK_QUERY_START)
            .map_err(|e| e.map(SlotAuthzConfigError::BlockQueryStart))?
            .clamp(0, 11);

        Ok(Self {
            calc,
            block_query_cutoff,
            block_query_start,
        })
    }
}
