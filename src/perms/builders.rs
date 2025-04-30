//! #Signet Quincey builder permissioning system.
//!
//! The permissioning system decides which builder can perform a certain action
//! at a given time. The permissioning system uses a simple round-robin design,
//! where each builder is allowed to perform an action at a specific slot.
//! Builders are permissioned based on their sub, which is present in the JWT
//! token they acquire from our OAuth service.

use crate::{
    perms::SlotAuthzConfig,
    utils::{
        calc::SlotCalculator,
        from_env::{FromEnv, FromEnvErr, FromEnvVar},
    },
};
use serde::{Deserialize, Deserializer};

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

/// An individual builder.
#[derive(Clone, Debug, serde::Deserialize)]
#[serde(from = "String")]
pub struct Builder {
    /// The sub of the builder.
    pub sub: String,
}

impl From<String> for Builder {
    fn from(sub: String) -> Self {
        Self { sub }
    }
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

impl FromEnvVar for Builder {
    type Error = std::convert::Infallible;

    fn from_env_var(env_var: &str) -> Result<Self, FromEnvErr<Self::Error>> {
        Ok(Self {
            sub: String::from_env_var(env_var)?,
        })
    }
}

/// Builders struct to keep track of the builders that are allowed to perform actions.
#[derive(Clone, Debug, serde::Deserialize, FromEnv)]
#[from_env(crate)]
pub struct Builders {
    /// The list of builders.
    ///
    /// This is configured in the environment variable `PERMISSIONED_BUILDERS`,
    /// as a list of comma-separated UUIDs.
    #[serde(deserialize_with = "deser_builders")]
    #[from_env(
        infallible,
        var = "BUILDERS",
        desc = "A comma-separated list of UUIDs representing the builders that are allowed to perform actions."
    )]
    pub builders: Vec<Builder>,

    /// The slot authorization configuration. See [`SlotAuthzConfig`] for more
    /// information and env vars
    config: SlotAuthzConfig,
}

fn deser_builders<'de, D>(deser: D) -> Result<Vec<Builder>, D::Error>
where
    D: Deserializer<'de>,
{
    let s = String::deserialize(deser)?;
    Ok(split_builders(&s))
}

fn split_builders(s: &str) -> Vec<Builder> {
    s.split(',').map(Builder::new).collect()
}

impl Builders {
    /// Create a new Builders struct.
    pub const fn new(builders: Vec<Builder>, config: SlotAuthzConfig) -> Self {
        Self { builders, config }
    }

    /// Get the calculator instance.
    pub const fn calc(&self) -> SlotCalculator {
        self.config.calc()
    }

    /// Get the slot authorization configuration.
    pub const fn config(&self) -> &SlotAuthzConfig {
        &self.config
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

    /// Check the query bounds for the current timestamp.
    fn check_query_bounds(&self) -> Result<(), BuilderPermissionError> {
        let current_slot_time = self.calc().current_timepoint_within_slot();
        if current_slot_time < self.config.block_query_start() {
            return Err(BuilderPermissionError::ActionAttemptTooEarly);
        }
        if current_slot_time > self.config.block_query_cutoff() {
            return Err(BuilderPermissionError::ActionAttemptTooLate);
        }
        Ok(())
    }

    /// Checks if a builder is allowed to perform an action.
    /// This is based on the current timestamp and the builder's sub. It's a
    /// round-robin design, where each builder is allowed to perform an action
    /// at a specific slot, and what builder is allowed changes with each slot.
    pub fn is_builder_permissioned(&self, sub: &str) -> Result<(), BuilderPermissionError> {
        self.check_query_bounds()?;

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

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn load_builders() {
        unsafe {
            std::env::set_var("BUILDERS", "0,1,2,3,4,5");

            std::env::set_var("START_TIMESTAMP", "1");
            std::env::set_var("SLOT_OFFSET", "0");
            std::env::set_var("SLOT_DURATION", "12");

            std::env::set_var("BLOCK_QUERY_START", "1");
            std::env::set_var("BLOCK_QUERY_CUTOFF", "11");
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
