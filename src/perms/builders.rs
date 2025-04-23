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

use crate::utils::from_env::{FromEnvErr, FromEnvVar};

/// The start timestamp for the permissioned builders, in seconds.
const EPOCH_START: u64 = 0;

/// Ethereum's slot time in seconds.
pub const ETHEREUM_SLOT_TIME: u64 = 12;

fn now() -> u64 {
    chrono::Utc::now().timestamp().try_into().unwrap()
}

/// Possible errors when permissioning a builder.
#[derive(Debug, thiserror::Error)]
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
}

impl Builders {
    /// Create a new Builders struct.
    pub const fn new(builders: Vec<Builder>) -> Self {
        Self { builders }
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
        ((timestamp - EPOCH_START) / ETHEREUM_SLOT_TIME) % self.builders.len() as u64
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
    pub fn is_builder_permissioned(
        &self,
        config: &crate::perms::SlotAuthzConfig,
        sub: &str,
    ) -> Result<(), BuilderPermissionError> {
        // Get the current timestamp.
        let curr_timestamp = now();

        // Calculate the current slot time, which is a number between 0 and 11.
        let current_slot_time = (curr_timestamp - config.chain_offset()) % ETHEREUM_SLOT_TIME;

        // Builders can only perform actions between the configured start and cutoff times, to prevent any timing games.
        if current_slot_time < config.block_query_start() {
            tracing::debug!("Action attempt too early");
            return Err(BuilderPermissionError::ActionAttemptTooEarly);
        }
        if current_slot_time > config.block_query_cutoff() {
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

impl FromIterator<Builder> for Builders {
    fn from_iter<T: IntoIterator<Item = Builder>>(iter: T) -> Self {
        Self::new(iter.into_iter().collect())
    }
}

impl FromEnvVar for Builders {
    type Error = BuilderPermissionError;

    fn from_env_var(env_var: &str) -> Result<Self, FromEnvErr<Self::Error>> {
        let s = String::from_env_var(env_var)
            .map_err(FromEnvErr::infallible_into::<BuilderPermissionError>)?;

        Ok(s.split(',').map(Builder::new).collect())
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn load_builders() {
        unsafe { std::env::set_var("TEST", "0,1,2,3,4,5") };

        let builders = Builders::from_env_var("TEST").unwrap();
        assert_eq!(builders.builder_at(0).sub, "0");
        assert_eq!(builders.builder_at(1).sub, "1");
        assert_eq!(builders.builder_at(2).sub, "2");
        assert_eq!(builders.builder_at(3).sub, "3");
        assert_eq!(builders.builder_at(4).sub, "4");
        assert_eq!(builders.builder_at(5).sub, "5");
    }
}
