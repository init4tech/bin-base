use crate::utils::{calc::SlotCalculator, from_env::FromEnv};

/// Configuration object that describes the slot time settings for a chain.
///
/// This struct is used to configure the slot authorization system
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize, FromEnv)]
#[from_env(crate)]
pub struct SlotAuthzConfig {
    /// A [`SlotCalculator`] instance that can be used to calculate the slot
    /// number for a given timestamp.
    calc: SlotCalculator,
    /// The block query cutoff time in seconds. This is the slot second after
    /// which requests will not be serviced. E.g. a value of 10 means that
    /// requests will not be serviced for the last 2 seconds of any given slot.
    ///
    /// On loading from env, the number will be clamped between 0 and 12, as
    /// the slot duration is 12 seconds.
    #[from_env(
        var = "BLOCK_QUERY_CUTOFF",
        desc = "The block query cutoff time in seconds."
    )]
    block_query_cutoff: u8,
    /// The block query start time in seconds. This is the slot second before
    /// which requests will not be serviced. E.g. a value of 1 means that
    /// requests will not be serviced for the first second of any given slot.
    ///
    /// On loading from env, the number will be clamped between 0 and 12, as
    /// the slot duration is 12 seconds.
    #[from_env(
        var = "BLOCK_QUERY_START",
        desc = "The block query start time in seconds."
    )]
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
