//! #Signet Quincey builder permissioning system.
//!
//! The permissioning system decides which builder can perform a certain action at a given time.
//! The permissioning system uses a simple round-robin design, where each builder is allowed to perform an action at a specific slot.
//! Builders are permissioned based on their sub, which is present in the JWT token they acquire from our OAuth service.
//! They are rotated every 12 seconds, which is Ethereum's slot time.
//! As the logic is timestamp based, the system is deterministic.
//!
//! For updating the currently permissioned builders,
//! Simply update the included `builders.json` file with the new builders.

use crate::{
    perms::{SlotAuthzConfig, SlotAuthzConfigError, SlotCalculator},
    utils::from_env::{FromEnv, FromEnvErr, FromEnvVar},
};

/// The builder list env var.
const BUILDERS: &str = "PERMISSIONED_BUILDERS";

/// Ethereum's slot time in seconds.
pub const ETHEREUM_SLOT_TIME: u64 = 12;

fn now() -> u64 {
    chrono::Utc::now().timestamp().try_into().unwrap()
}

/// Possible errors when permissioning a builder.
#[derive(Debug, thiserror::Error, Clone, Copy, PartialEq, Eq)]
pub enum BuilderPermissionError {
    /// Action attempt too early.
    #[error("action attempt too early")]
    ActionAttemptTooEarly,

    /// Action attempt too late.
    #[error("action attempt too late")]
    ActionAttemptTooLate,

    /// Builder not permissioned for this slot.
    #[error("builder not permissioned for this slot")]
    NotPermissioned,
}

/// Possible errors when loading the builder configuration.
#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq)]
pub enum BuilderConfigError {
    /// Error loading the environment variable.
    #[error(
        "failed to parse environment variable. Expected a comma-seperated list of UUIDs. Got: {input}"
    )]
    ParseError {
        /// The environment variable name.
        env_var: String,
        /// The contents of the environment variable.
        input: String,
    },

    /// Error loading the slot authorization configuration.
    #[error(transparent)]
    SlotAutzConfig(#[from] SlotAuthzConfigError),
}

/// An individual builder.
#[derive(Clone, Debug)]
pub struct Builder {
    /// The sub of the builder.
    pub sub: String,
}

impl Builder {
    /// Create a new builder.
    pub fn new(sub: impl AsRef<str>) -> Self {
        Self {
            sub: sub.as_ref().to_owned(),
        }
    }
    /// Get the sub of the builder.
    #[allow(clippy::missing_const_for_fn)] // false positive, non-const deref
    pub fn sub(&self) -> &str {
        &self.sub
    }
}

/// Builders struct to keep track of the builders that are allowed to perform actions.
#[derive(Clone, Debug)]
pub struct Builders {
    /// The list of builders.
    pub builders: Vec<Builder>,

    /// The slot authorization configuration.
    config: SlotAuthzConfig,
}

impl Builders {
    /// Create a new Builders struct.
    pub const fn new(builders: Vec<Builder>, config: SlotAuthzConfig) -> Self {
        Self { builders, config }
    }

    /// Get the calculator instance.
    pub fn calc(&self) -> SlotCalculator {
        self.config.calc()
    }

    /// Get the builder at a specific index.
    ///
    /// # Panics
    ///
    /// Panics if the index is out of bounds from the builders array.
    pub fn builder_at(&self, index: usize) -> &Builder {
        &self.builders[index]
    }

    /// Get the builder permissioned at a specific timestamp.
    pub fn builder_at_timestamp(&self, timestamp: u64) -> &Builder {
        self.builder_at(self.index(timestamp) as usize)
    }

    /// Get the index of the builder that is allowed to sign a block for a
    /// particular timestamp.
    pub fn index(&self, timestamp: u64) -> u64 {
        self.config.calc().calculate_slot(timestamp) % self.builders.len() as u64
    }

    /// Get the index of the builder that is allowed to sign a block at the
    /// current timestamp.
    pub fn index_now(&self) -> u64 {
        self.index(now())
    }

    /// Get the builder that is allowed to sign a block at the current timestamp.
    pub fn current_builder(&self) -> &Builder {
        self.builder_at(self.index_now() as usize)
    }

    /// Checks if a builder is allowed to perform an action.
    /// This is based on the current timestamp and the builder's sub. It's a
    /// round-robin design, where each builder is allowed to perform an action
    /// at a specific slot, and what builder is allowed changes with each slot.
    pub fn is_builder_permissioned(&self, sub: &str) -> Result<(), BuilderPermissionError> {
        // Get the current timestamp.

        // Calculate the current slot time, which is a number between 0 and 11.
        let current_slot_time = self.calc().current_timepoint_within_slot();

        // Builders can only perform actions between the configured start and cutoff times, to prevent any timing games.
        if current_slot_time < self.config.block_query_start() {
            tracing::debug!("Action attempt too early");
            return Err(BuilderPermissionError::ActionAttemptTooEarly);
        }
        if current_slot_time > self.config.block_query_cutoff() {
            tracing::debug!("Action attempt too late");
            return Err(BuilderPermissionError::ActionAttemptTooLate);
        }

        if sub != self.current_builder().sub {
            tracing::debug!(
                builder = %sub,
                permissioned_builder = %self.current_builder().sub,
                "Builder not permissioned for this slot"
            );
            return Err(BuilderPermissionError::NotPermissioned);
        }

        Ok(())
    }
}

impl FromEnv for Builders {
    type Error = BuilderConfigError;

    fn from_env() -> Result<Self, FromEnvErr<Self::Error>> {
        let s = String::from_env_var(BUILDERS)
            .map_err(FromEnvErr::infallible_into::<BuilderConfigError>)?;
        let builders = s.split(',').map(Builder::new).collect();

        let config = SlotAuthzConfig::from_env().map_err(FromEnvErr::from)?;

        Ok(Self { builders, config })
    }
}

#[cfg(test)]
mod test {

    use super::*;
    use crate::perms;

    #[test]
    fn load_builders() {
        unsafe {
            std::env::set_var(BUILDERS, "0,1,2,3,4,5");

            std::env::set_var(perms::calc::START_TIMESTAMP, "1");
            std::env::set_var(perms::calc::SLOT_OFFSET, "0");
            std::env::set_var(perms::calc::SLOT_DURATION, "12");

            std::env::set_var(perms::config::BLOCK_QUERY_START, "1");
            std::env::set_var(perms::config::BLOCK_QUERY_CUTOFF, "11");
        };

        let builders = Builders::from_env().unwrap();
        assert_eq!(builders.builder_at(0).sub, "0");
        assert_eq!(builders.builder_at(1).sub, "1");
        assert_eq!(builders.builder_at(2).sub, "2");
        assert_eq!(builders.builder_at(3).sub, "3");
        assert_eq!(builders.builder_at(4).sub, "4");
        assert_eq!(builders.builder_at(5).sub, "5");

        assert_eq!(builders.calc().slot_offset(), 0);
        assert_eq!(builders.calc().slot_duration(), 12);
        assert_eq!(builders.calc().start_timestamp(), 1);

        assert_eq!(builders.config.block_query_start(), 1);
        assert_eq!(builders.config.block_query_cutoff(), 11);
    }
}
