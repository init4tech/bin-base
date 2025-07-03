use crate::utils::from_env::FromEnv;

/// A slot calculator, which can calculate the slot number for a given
/// timestamp.
///
/// ## Behavior
///
/// Chain slot behavior is a bit unintuitive, particularly for chains that
/// have a merge or chains that have missed slots at the start of the chain
/// (i.e. Ethereum and its testnets).
///
/// Each header occupies a slot, but not all slots contain headers.
/// Headers contain the timestamp at the END of their respective slot.
///
/// Chains _start_ with a first header, which contains a timestamp (the
/// `start_timestamp`) and occupies the initial slot (the `slot_offset`).
/// The `start_timestamp` is therefore the END of the initial slot, and the
/// BEGINNING of the next slot. I.e. if the initial slot is 0, then the start
/// of slot 1 is the `start_timestamp` and the end of slot 1 is
/// `start_timestamp + slot_duration`.
#[derive(Debug, Copy, Clone, PartialEq, Eq, serde::Deserialize, FromEnv)]
#[from_env(crate)]
pub struct SlotCalculator {
    /// The start timestamp. This is the timestamp of the header to start the
    /// PoS chain. That header occupies a specific slot (the `slot_offset`). The
    /// `start_timestamp` is the END of that slot.
    #[from_env(
        var = "START_TIMESTAMP",
        desc = "The start timestamp of the chain in seconds"
    )]
    start_timestamp: u64,

    /// This is the number of the slot containing the block which contains the
    /// `start_timestamp`.
    ///
    /// This is needed for chains that contain a merge (like Ethereum Mainnet),
    /// or for chains with missed slots at the start of the chain (like
    /// Holesky).
    #[from_env(
        var = "SLOT_OFFSET",
        desc = "The number of the slot containing the start timestamp"
    )]
    slot_offset: u64,

    /// The slot duration (in seconds).
    #[from_env(
        var = "SLOT_DURATION",
        desc = "The slot duration of the chain in seconds"
    )]
    slot_duration: u64,
}

impl SlotCalculator {
    /// Creates a new slot calculator.
    pub const fn new(start_timestamp: u64, slot_offset: u64, slot_duration: u64) -> Self {
        Self {
            start_timestamp,
            slot_offset,
            slot_duration,
        }
    }

    /// Creates a new slot calculator for Holesky.
    pub const fn holesky() -> Self {
        // begin slot calculation for Holesky from block number 1, slot number 2, timestamp 1695902424
        // because of a strange 324 second gap between block 0 and 1 which
        // should have been 27 slots, but which is recorded as 2 slots in chain data
        Self {
            start_timestamp: 1695902424,
            slot_offset: 2,
            slot_duration: 12,
        }
    }

    /// Creates a new slot calculator for Pecorino host network.
    pub const fn pecorino_host() -> Self {
        Self {
            start_timestamp: 1740681556,
            slot_offset: 0,
            slot_duration: 12,
        }
    }

    /// Creates a new slot calculator for Ethereum mainnet.
    pub const fn mainnet() -> Self {
        Self {
            start_timestamp: 1663224179,
            slot_offset: 4700013,
            slot_duration: 12,
        }
    }

    /// Calculates the slot that contains a given timestamp.
    ///
    /// Returns `None` if the timestamp is before the chain's start timestamp.
    pub const fn time_to_slot(&self, timestamp: u64) -> Option<u64> {
        let Some(elapsed) = timestamp.checked_sub(self.start_timestamp) else {
            return None;
        };
        let slots = (elapsed / self.slot_duration) + 1;
        Some(slots + self.slot_offset)
    }

    /// Calculates how many seconds a given timestamp is into its containing
    /// slot.
    ///
    /// Returns `None` if the timestamp is before the chain's start.
    pub const fn point_within_slot(&self, timestamp: u64) -> Option<u64> {
        let Some(offset) = timestamp.checked_sub(self.slot_utc_offset()) else {
            return None;
        };
        Some(offset % self.slot_duration)
    }

    /// Calculates how many seconds a given timestamp is into a given slot.
    /// Returns `None` if the timestamp is not within the slot.
    pub const fn checked_point_within_slot(&self, slot: u64, timestamp: u64) -> Option<u64> {
        let calculated = self.time_to_slot(timestamp);
        if calculated.is_none() || calculated.unwrap() != slot {
            return None;
        }
        self.point_within_slot(timestamp)
    }

    /// Calculates the start and end timestamps for a given slot
    pub const fn slot_window(&self, slot_number: u64) -> std::ops::Range<u64> {
        let end_of_slot =
            ((slot_number - self.slot_offset) * self.slot_duration) + self.start_timestamp;
        let start_of_slot = end_of_slot - self.slot_duration;
        start_of_slot..end_of_slot
    }

    /// Calculates the slot window for the slot that corresponds to the given
    /// timestamp.
    ///
    /// Returns `None` if the timestamp is before the chain's start timestamp.
    pub const fn slot_window_for_timestamp(&self, timestamp: u64) -> Option<std::ops::Range<u64>> {
        let Some(slot) = self.time_to_slot(timestamp) else {
            return None;
        };
        Some(self.slot_window(slot))
    }

    /// The current slot number.
    ///
    /// Returns `None` if the current time is before the chain's start
    /// timestamp.
    pub fn current_slot(&self) -> Option<u64> {
        self.time_to_slot(chrono::Utc::now().timestamp() as u64)
    }

    /// The current number of seconds into the slot.
    pub fn current_point_within_slot(&self) -> Option<u64> {
        self.point_within_slot(chrono::Utc::now().timestamp() as u64)
    }

    /// The timestamp of the first PoS block in the chain.
    pub const fn start_timestamp(&self) -> u64 {
        self.start_timestamp
    }

    /// The slot number of the first PoS block in the chain.
    pub const fn slot_offset(&self) -> u64 {
        self.slot_offset
    }

    /// The slot duration, usually 12 seconds.
    pub const fn slot_duration(&self) -> u64 {
        self.slot_duration
    }

    /// The offset in seconds between UTC time and slot mining times
    const fn slot_utc_offset(&self) -> u64 {
        self.start_timestamp % self.slot_duration
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_slot_calculations() {
        let calculator = SlotCalculator::new(12, 0, 12);
        assert_eq!(calculator.time_to_slot(0), None);

        assert_eq!(calculator.time_to_slot(1), Some(13));
        assert_eq!(calculator.time_to_slot(11), Some(13));
        assert_eq!(calculator.time_to_slot(12), Some(13));

        assert_eq!(calculator.time_to_slot(13), Some(14));
        assert_eq!(calculator.time_to_slot(23), Some(14));
        assert_eq!(calculator.time_to_slot(24), Some(14));

        assert_eq!(calculator.time_to_slot(25), Some(15));
        assert_eq!(calculator.time_to_slot(35), Some(15));
        assert_eq!(calculator.time_to_slot(36), Some(15));
    }

    #[test]
    fn test_holesky_slot_calculations() {
        let calculator = SlotCalculator::holesky();

        // Just before the start timestamp
        let just_before = calculator.start_timestamp - 1;
        assert_eq!(calculator.time_to_slot(just_before), None);

        // Timestamp 17
        assert_eq!(calculator.time_to_slot(17), None);

        // block 1 == slot 2 == timestamp 1695902424
        // timestamp 1695902424 == slot 2
        assert_eq!(calculator.time_to_slot(1695902424), Some(2));
        // the next second, timestamp 1695902425 == slot 3
        assert_eq!(calculator.time_to_slot(1695902425), Some(3));

        // block 3557085 == slot 3919127 == timestamp 1742931924
        // timestamp 1742931924 == slot 3919127
        assert_eq!(calculator.time_to_slot(1742931924), Some(3919127));
        // the next second, timestamp 1742931925 == slot 3919128
        assert_eq!(calculator.time_to_slot(1742931925), Some(3919128));
    }

    #[test]
    fn test_holesky_slot_timepoint_calculations() {
        let calculator = SlotCalculator::holesky();
        // calculate timepoint in slot
        assert_eq!(calculator.point_within_slot(1695902424), Some(0));
        assert_eq!(calculator.point_within_slot(1695902425), Some(1));
        assert_eq!(calculator.point_within_slot(1695902435), Some(11));
        assert_eq!(calculator.point_within_slot(1695902436), Some(0));
    }

    #[test]
    fn test_holesky_slot_window() {
        let calculator = SlotCalculator::holesky();
        // calculate slot window
        assert_eq!(calculator.slot_window(2), 1695902412..1695902424);
        assert_eq!(calculator.slot_window(3), 1695902424..1695902436);
    }

    #[test]
    fn test_mainnet_slot_calculations() {
        let calculator = SlotCalculator::mainnet();

        // Just before the start timestamp
        let just_before = calculator.start_timestamp - 1;
        assert_eq!(calculator.time_to_slot(just_before), None);

        // Timestamp 17
        assert_eq!(calculator.time_to_slot(17), None);

        assert_eq!(calculator.time_to_slot(1663224179), Some(4700013));
        assert_eq!(calculator.time_to_slot(1663224180), Some(4700014));

        assert_eq!(calculator.time_to_slot(1738863035), Some(11003251));
        assert_eq!(calculator.time_to_slot(1738866239), Some(11003518));
        assert_eq!(calculator.time_to_slot(1738866227), Some(11003517));
    }

    #[test]
    fn test_mainnet_slot_timepoint_calculations() {
        let calculator = SlotCalculator::mainnet();
        // calculate timepoint in slot
        assert_eq!(calculator.point_within_slot(1663224179), Some(0));
        assert_eq!(calculator.point_within_slot(1663224180), Some(1));
        assert_eq!(calculator.point_within_slot(1663224190), Some(11));
        assert_eq!(calculator.point_within_slot(1663224191), Some(0));
    }

    #[test]
    fn test_ethereum_slot_window() {
        let calculator = SlotCalculator::mainnet();
        // calculate slot window
        assert_eq!(calculator.slot_window(4700013), (1663224167..1663224179));
        assert_eq!(calculator.slot_window(4700014), (1663224179..1663224191));
    }

    #[test]
    fn slot_boundaries() {
        let calculator = SlotCalculator::new(0, 0, 2);

        // Check the boundaries of slots
        assert_eq!(calculator.time_to_slot(0), Some(1));
        assert_eq!(calculator.time_to_slot(1), Some(1));
        assert_eq!(calculator.time_to_slot(2), Some(2));
        assert_eq!(calculator.time_to_slot(3), Some(2));
        assert_eq!(calculator.time_to_slot(4), Some(3));
        assert_eq!(calculator.time_to_slot(5), Some(3));
        assert_eq!(calculator.time_to_slot(6), Some(4));

        let calculator = SlotCalculator::new(12, 0, 12);

        // Check the boundaries of slots
        assert_eq!(calculator.time_to_slot(0), None);
        assert_eq!(calculator.time_to_slot(11), None);
        assert_eq!(calculator.time_to_slot(12), Some(1));
        assert_eq!(calculator.time_to_slot(13), Some(1));
        assert_eq!(calculator.time_to_slot(23), Some(1));
        assert_eq!(calculator.time_to_slot(24), Some(2));
        assert_eq!(calculator.time_to_slot(25), Some(2));
        assert_eq!(calculator.time_to_slot(35), Some(2));

        let calculator = SlotCalculator::new(12, 1, 12);

        assert_eq!(calculator.time_to_slot(0), None);
        assert_eq!(calculator.time_to_slot(11), None);
        assert_eq!(calculator.time_to_slot(12), Some(2));
        assert_eq!(calculator.time_to_slot(13), Some(2));
        assert_eq!(calculator.time_to_slot(23), Some(2));
        assert_eq!(calculator.time_to_slot(24), Some(3));
        assert_eq!(calculator.time_to_slot(25), Some(3));
        assert_eq!(calculator.time_to_slot(35), Some(3));
    }
}
