use crate::utils::from_env::FromEnv;
use signet_constants::KnownChains;
use std::str::FromStr;

/// A slot calculator, which can calculate slot numbers, windows, and offsets
/// for a given chain.
///
/// ## Typing
///
/// `slots` are indices, and use `usize` for their type.
/// `timestamps` are in Unix Epoch seconds, and use `u64` for their type.
///
/// It is recommended that literal integers passed to these functions be
/// explicitly typed, e.g. `0u64`, `12usize`, etc., to avoid confusion in
/// calling code.
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
///
/// For a given slot, we normalize its number to `n` by subtracting the slot
/// offset. `n` is therefore in the range `1..`.
///
/// - `n = normalized(slot) = slot - slot_offset`.
///
/// As such, we can define the `slot_start(n)` as
/// - `slot_start(n) = (n - 1) * slot_duration + start_timestamp`
///
/// and `slot_end(n)` as
/// - `slot_end(n) = n * slot_duration + start_timestamp`
///
/// The slot `n` contains the range of timestamps:
/// - `slot_window(n) = slot_start(n)..slot_end(n)`
///
/// To calculate the slot number `n` for a given timestamp `t`, we can use the
/// following formula:
/// - `slot_for(t) = ((t - start_timestamp) / slot_duration) + slot_offset + 1`
///
/// The `+ 1` is added because the first slot is the slot at `slot_offset`,
/// which ENDS at `start_timestamp`. I.e. a timestamp at `start_timestamp` is
/// in slot `slot_offset + 1`.
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
    slot_offset: usize,

    /// The slot duration (in seconds).
    #[from_env(
        var = "SLOT_DURATION",
        desc = "The slot duration of the chain in seconds"
    )]
    slot_duration: u64,
}

impl SlotCalculator {
    /// Creates a new slot calculator.
    pub const fn new(start_timestamp: u64, slot_offset: usize, slot_duration: u64) -> Self {
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
            start_timestamp: 1754584265,
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

    /// The timestamp of the first PoS block in the chain.
    pub const fn start_timestamp(&self) -> u64 {
        self.start_timestamp
    }

    /// The slot number of the first PoS block in the chain.
    pub const fn slot_offset(&self) -> usize {
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

    /// Calculates the slot that contains a given timestamp.
    ///
    /// Returns `None` if the timestamp is before the chain's start timestamp.
    pub const fn slot_containing(&self, timestamp: u64) -> Option<usize> {
        let Some(elapsed) = timestamp.checked_sub(self.start_timestamp) else {
            return None;
        };
        let slots = (elapsed / self.slot_duration) + 1;
        Some(slots as usize + self.slot_offset)
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
    pub const fn checked_point_within_slot(&self, slot: usize, timestamp: u64) -> Option<u64> {
        let calculated = self.slot_containing(timestamp);
        if calculated.is_none() || calculated.unwrap() != slot {
            return None;
        }
        self.point_within_slot(timestamp)
    }

    /// Calculates the start and end timestamps for a given slot
    pub const fn slot_window(&self, slot_number: usize) -> std::ops::Range<u64> {
        let end_of_slot =
            ((slot_number - self.slot_offset) as u64 * self.slot_duration) + self.start_timestamp;
        let start_of_slot = end_of_slot - self.slot_duration;
        start_of_slot..end_of_slot
    }

    /// Calculates the start timestamp of a given slot.
    pub const fn slot_start(&self, slot_number: usize) -> u64 {
        self.slot_window(slot_number).start
    }

    /// Calculates the end timestamp of a given slot.
    pub const fn slot_end(&self, slot_number: usize) -> u64 {
        self.slot_window(slot_number).end
    }

    /// Calculate the timestamp that will appear in the header of the block at
    /// the given slot number (if any block is produced). This is an alias for
    /// [`Self::slot_end`].
    #[inline(always)]
    pub const fn slot_timestamp(&self, slot_number: usize) -> u64 {
        // The timestamp of the slot is the end of the slot window.
        self.slot_end(slot_number)
    }

    /// Calculates the slot window for the slot that contains to the given
    /// timestamp. Slot windows are ranges `start..end`, where `start` is the
    /// end timestamp of the slot and `end` is `start + slot_duration`.
    ///
    /// Returns `None` if the timestamp is before the chain's start timestamp.
    pub const fn slot_window_for_timestamp(&self, timestamp: u64) -> Option<std::ops::Range<u64>> {
        let Some(slot) = self.slot_containing(timestamp) else {
            return None;
        };
        Some(self.slot_window(slot))
    }

    /// Calcuates the start timestamp for the slot that contains the given
    /// timestamp.
    pub const fn slot_start_for_timestamp(&self, timestamp: u64) -> Option<u64> {
        if let Some(window) = self.slot_window_for_timestamp(timestamp) {
            Some(window.start)
        } else {
            None
        }
    }

    /// Calculates the end timestamp for the slot that contains to the given
    /// timestamp.
    pub const fn slot_end_for_timestamp(&self, timestamp: u64) -> Option<u64> {
        if let Some(window) = self.slot_window_for_timestamp(timestamp) {
            Some(window.end)
        } else {
            None
        }
    }

    /// The current slot number.
    ///
    /// Returns `None` if the current time is before the chain's start
    /// timestamp.
    pub fn current_slot(&self) -> Option<usize> {
        self.slot_containing(chrono::Utc::now().timestamp() as u64)
    }

    /// The current number of seconds into the slot.
    pub fn current_point_within_slot(&self) -> Option<u64> {
        self.point_within_slot(chrono::Utc::now().timestamp() as u64)
    }

    /// Calculates the slot that starts at the given timestamp.
    /// Returns `None` if the timestamp is not a slot boundary.
    /// Returns `None` if the timestamp is before the chain's start timestamp.
    pub fn slot_starting_at(&self, timestamp: u64) -> Option<usize> {
        let elapsed = timestamp.checked_sub(self.start_timestamp)?;

        if elapsed % self.slot_duration != 0 {
            return None;
        }

        self.slot_containing(timestamp)
    }

    /// Calculates the slot that ends at the given timestamp.
    /// Returns `None` if the timestamp is not a slot boundary.
    /// Returns `None` if the timestamp is before the chain's start timestamp.
    pub fn slot_ending_at(&self, timestamp: u64) -> Option<usize> {
        let elapsed = timestamp.checked_sub(self.start_timestamp)?;

        if elapsed % self.slot_duration != 0 {
            return None;
        }

        self.slot_containing(timestamp)
            .and_then(|slot| slot.checked_sub(1))
    }
}

impl From<KnownChains> for SlotCalculator {
    fn from(value: KnownChains) -> Self {
        match value {
            KnownChains::Pecorino => SlotCalculator::pecorino_host(),
        }
    }
}

impl FromStr for SlotCalculator {
    type Err = signet_constants::ParseChainError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(SlotCalculator::from(KnownChains::from_str(s)?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    impl SlotCalculator {
        #[track_caller]
        fn assert_contains(&self, slot: usize, timestamp: u64) {
            assert_eq!(self.slot_containing(timestamp), Some(slot));
            assert!(self.slot_window(slot).contains(&timestamp));
        }
    }

    #[test]
    fn test_basic_slot_calculations() {
        let calculator = SlotCalculator::new(12, 0, 12);
        assert_eq!(calculator.slot_ending_at(0), None);
        assert_eq!(calculator.slot_containing(0), None);
        assert_eq!(calculator.slot_containing(1), None);
        assert_eq!(calculator.slot_containing(11), None);

        assert_eq!(calculator.slot_ending_at(11), None);
        assert_eq!(calculator.slot_ending_at(12), Some(0));
        assert_eq!(calculator.slot_starting_at(12), Some(1));
        assert_eq!(calculator.slot_containing(12), Some(1));
        assert_eq!(calculator.slot_containing(13), Some(1));
        assert_eq!(calculator.slot_starting_at(13), None);
        assert_eq!(calculator.slot_containing(23), Some(1));
        assert_eq!(calculator.slot_ending_at(23), None);

        assert_eq!(calculator.slot_ending_at(24), Some(1));
        assert_eq!(calculator.slot_starting_at(24), Some(2));
        assert_eq!(calculator.slot_containing(24), Some(2));
        assert_eq!(calculator.slot_containing(25), Some(2));
        assert_eq!(calculator.slot_containing(35), Some(2));

        assert_eq!(calculator.slot_containing(36), Some(3));
    }

    #[test]
    fn test_holesky_slot_calculations() {
        let calculator = SlotCalculator::holesky();

        // Just before the start timestamp
        let just_before = calculator.start_timestamp - 1;
        assert_eq!(calculator.slot_containing(just_before), None);

        // Timestamp 17
        assert_eq!(calculator.slot_containing(17), None);

        // block 1 == slot 2 == timestamp 1695902424
        // timestamp 1695902424 == slot 3 is in slot 3
        calculator.assert_contains(3, 1695902424);

        // the next second, timestamp 1695902425 == slot 3
        calculator.assert_contains(3, 1695902425);

        // block 3557085 == slot 3919127 == timestamp 1742931924
        // timestamp 1742931924 == slot 3919127
        calculator.assert_contains(3919128, 1742931924);
        // the next second, timestamp 1742931925 == slot 3919128
        calculator.assert_contains(3919128, 1742931925);
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
        assert_eq!(calculator.slot_containing(just_before), None);

        // Timestamp 17
        assert_eq!(calculator.slot_containing(17), None);

        // 1663224179 - Sep-15-2022 06:42:59 AM +UTC
        // https://beaconscan.com/slot/4700013
        calculator.assert_contains(4700014, 1663224179);
        calculator.assert_contains(4700014, 1663224180);

        // https://beaconscan.com/slot/11003251
        calculator.assert_contains(11003252, 1738863035);
        // https://beaconscan.com/slot/11003518
        calculator.assert_contains(11003519, 1738866239);
        // https://beaconscan.com/slot/11003517
        calculator.assert_contains(11003518, 1738866227);
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
        calculator.assert_contains(1, 0);
        calculator.assert_contains(1, 1);
        calculator.assert_contains(2, 2);
        calculator.assert_contains(2, 3);
        calculator.assert_contains(3, 4);
        calculator.assert_contains(3, 5);
        calculator.assert_contains(4, 6);

        let calculator = SlotCalculator::new(12, 0, 12);

        // Check the boundaries of slots
        assert_eq!(calculator.slot_containing(0), None);
        assert_eq!(calculator.slot_containing(11), None);
        calculator.assert_contains(1, 12);
        calculator.assert_contains(1, 13);
        calculator.assert_contains(1, 23);
        calculator.assert_contains(2, 24);
        calculator.assert_contains(2, 25);
        calculator.assert_contains(2, 35);

        let calculator = SlotCalculator::new(12, 1, 12);

        assert_eq!(calculator.slot_containing(0), None);
        assert_eq!(calculator.slot_containing(11), None);
        assert_eq!(calculator.slot_containing(12), Some(2));
        assert_eq!(calculator.slot_containing(13), Some(2));
        assert_eq!(calculator.slot_containing(23), Some(2));
        assert_eq!(calculator.slot_containing(24), Some(3));
        assert_eq!(calculator.slot_containing(25), Some(3));
        assert_eq!(calculator.slot_containing(35), Some(3));
    }
}
