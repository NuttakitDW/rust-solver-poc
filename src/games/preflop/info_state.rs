//! Information state for poker CFR.
//!
//! This module defines the information state representation used by the CFR solver.
//! The information state captures what a player knows at a decision point,
//! abstracted via card buckets for tractability.

use super::card::Street;
use super::state::{PokerState, HUPosition};
use super::abstraction::CardAbstraction;
use crate::cfr::game::InfoState;
use std::fmt;

/// Information state for a poker player.
///
/// Contains:
/// - Player position
/// - Current street
/// - Abstracted hand bucket
/// - Action history
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PokerInfoState {
    /// Player position (SB=0, BB=1)
    pub position: u8,
    /// Current street (0-4)
    pub street: u8,
    /// Abstracted hand bucket
    pub bucket: u16,
    /// Action history string
    pub history: String,
}

impl PokerInfoState {
    /// Create a new info state.
    pub fn new(position: HUPosition, street: Street, bucket: u16, history: String) -> Self {
        Self {
            position: position.index() as u8,
            street: street.index() as u8,
            bucket,
            history,
        }
    }

    /// Create from game state using card abstraction.
    pub fn from_state(state: &PokerState, abstraction: &CardAbstraction) -> Option<Self> {
        let pos = state.to_act?;
        let hole_cards = state.hand(pos)?;

        let bucket = abstraction.get_bucket(hole_cards, &state.board);
        let history = state.full_history_string();

        Some(Self::new(pos, state.street, bucket, history))
    }

    /// Get the position.
    pub fn position(&self) -> HUPosition {
        HUPosition::from_index(self.position as usize)
    }

    /// Get the street.
    pub fn street(&self) -> Street {
        match self.street {
            0 => Street::Preflop,
            1 => Street::Flop,
            2 => Street::Turn,
            3 => Street::River,
            _ => Street::Showdown,
        }
    }
}

impl InfoState for PokerInfoState {
    fn key(&self) -> String {
        // Format: P{position}S{street}B{bucket}|{history}
        // Example: P0S1B523|R300-C|X-B132-C
        format!("P{}S{}B{}|{}", self.position, self.street, self.bucket, self.history)
    }
}

impl fmt::Display for PokerInfoState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{} bucket {} [{}]",
            if self.position == 0 { "SB" } else { "BB" },
            self.street(),
            self.bucket,
            self.history
        )
    }
}

/// Builder for info states with configurable abstraction.
#[derive(Debug, Clone)]
pub struct InfoStateBuilder {
    abstraction: CardAbstraction,
}

impl InfoStateBuilder {
    /// Create a new builder with default abstraction.
    pub fn new() -> Self {
        Self {
            abstraction: CardAbstraction::new(),
        }
    }

    /// Create with custom abstraction.
    pub fn with_abstraction(abstraction: CardAbstraction) -> Self {
        Self { abstraction }
    }

    /// Build info state from game state.
    pub fn build(&self, state: &PokerState) -> Option<PokerInfoState> {
        PokerInfoState::from_state(state, &self.abstraction)
    }

    /// Get reference to the abstraction.
    pub fn abstraction(&self) -> &CardAbstraction {
        &self.abstraction
    }
}

impl Default for InfoStateBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Compact info state representation for memory efficiency.
/// Uses fixed-size encoding instead of dynamic strings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CompactInfoState {
    /// Packed data: position (1 bit) | street (3 bits) | bucket (12 bits)
    packed: u16,
    /// History hash (for uniqueness)
    history_hash: u64,
}

impl CompactInfoState {
    /// Create a compact info state.
    pub fn new(position: HUPosition, street: Street, bucket: u16, history_hash: u64) -> Self {
        let pos_bits = (position.index() as u16) << 15;
        let street_bits = (street.index() as u16 & 0x7) << 12;
        let bucket_bits = bucket & 0xFFF;

        Self {
            packed: pos_bits | street_bits | bucket_bits,
            history_hash,
        }
    }

    /// Create from full info state.
    pub fn from_info_state(info: &PokerInfoState) -> Self {
        use std::hash::{Hash, Hasher};
        use std::collections::hash_map::DefaultHasher;

        let mut hasher = DefaultHasher::new();
        info.history.hash(&mut hasher);
        let history_hash = hasher.finish();

        Self::new(info.position(), info.street(), info.bucket, history_hash)
    }

    /// Get position.
    pub fn position(&self) -> HUPosition {
        HUPosition::from_index((self.packed >> 15) as usize)
    }

    /// Get street index.
    pub fn street_index(&self) -> usize {
        ((self.packed >> 12) & 0x7) as usize
    }

    /// Get bucket.
    pub fn bucket(&self) -> u16 {
        self.packed & 0xFFF
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::games::preflop::card::HoleCards;
    use crate::games::preflop::action::PokerAction;

    #[test]
    fn test_info_state_key() {
        let info = PokerInfoState::new(
            HUPosition::SB,
            Street::Flop,
            523,
            "R300-C|X".to_string(),
        );

        let key = info.key();
        assert!(key.starts_with("P0S1B523"));
        assert!(key.contains("R300-C|X"));
    }

    #[test]
    fn test_info_state_from_game_state() {
        let abstraction = CardAbstraction::new();

        let sb_hand = HoleCards::from_str("AsAd").unwrap();
        let bb_hand = HoleCards::from_str("KhKs").unwrap();

        let state = PokerState::new_hu([50.0, 50.0], 0.5, 1.0)
            .with_hands(sb_hand, bb_hand);

        let info = PokerInfoState::from_state(&state, &abstraction).unwrap();

        assert_eq!(info.position(), HUPosition::SB);
        assert_eq!(info.street(), Street::Preflop);
        // AA should have bucket 12 (hand class for AA)
        assert_eq!(info.bucket, 12);
    }

    #[test]
    fn test_info_state_builder() {
        let builder = InfoStateBuilder::new();

        let sb_hand = HoleCards::from_str("7c2d").unwrap();
        let bb_hand = HoleCards::from_str("AhKs").unwrap();

        let state = PokerState::new_hu([50.0, 50.0], 0.5, 1.0)
            .with_hands(sb_hand, bb_hand);

        let info = builder.build(&state).unwrap();

        // 72o should have a low hand class bucket
        assert!(info.bucket < 169);
    }

    #[test]
    fn test_info_state_changes_with_actions() {
        let builder = InfoStateBuilder::new();

        let sb_hand = HoleCards::from_str("AsAd").unwrap();
        let bb_hand = HoleCards::from_str("KhKs").unwrap();

        let mut state = PokerState::new_hu([50.0, 50.0], 0.5, 1.0)
            .with_hands(sb_hand, bb_hand);

        let info1 = builder.build(&state).unwrap();

        // SB raises
        state = state.apply(PokerAction::Raise(300));
        let info2 = builder.build(&state).unwrap();

        // Info state should be different (different player, different history)
        assert_ne!(info1.key(), info2.key());
        assert_eq!(info2.position(), HUPosition::BB);
    }

    #[test]
    fn test_compact_info_state() {
        let info = PokerInfoState::new(
            HUPosition::BB,
            Street::Turn,
            1000,
            "R300-C|X-B132|C".to_string(),
        );

        let compact = CompactInfoState::from_info_state(&info);

        assert_eq!(compact.position(), HUPosition::BB);
        assert_eq!(compact.street_index(), 2); // Turn
        assert_eq!(compact.bucket(), 1000);
    }

    #[test]
    fn test_info_state_uniqueness() {
        // Same bucket, different history should produce different keys
        let info1 = PokerInfoState::new(
            HUPosition::SB,
            Street::Flop,
            100,
            "R300-C|X".to_string(),
        );

        let info2 = PokerInfoState::new(
            HUPosition::SB,
            Street::Flop,
            100,
            "C|X-B132".to_string(),
        );

        assert_ne!(info1.key(), info2.key());

        // Same history, different bucket should also be different
        let info3 = PokerInfoState::new(
            HUPosition::SB,
            Street::Flop,
            200,
            "R300-C|X".to_string(),
        );

        assert_ne!(info1.key(), info3.key());
    }
}
