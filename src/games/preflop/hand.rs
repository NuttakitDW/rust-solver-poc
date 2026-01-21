//! Hand range utilities.
//!
//! This module provides utilities for working with poker hand ranges,
//! including range notation parsing and combo enumeration.

use super::card::{Card, HoleCards};
use super::abstraction::HandClass;
use std::collections::HashSet;

/// A poker range represented as a set of hand classes.
#[derive(Debug, Clone, Default)]
pub struct Range {
    /// Bitset of included hand classes (169 bits)
    classes: [u64; 3],
}

impl Range {
    /// Create an empty range.
    pub fn empty() -> Self {
        Self { classes: [0; 3] }
    }

    /// Create a range with all hands.
    pub fn all() -> Self {
        let mut range = Self::empty();
        for i in 0..169 {
            range.add_class(i);
        }
        range
    }

    /// Add a hand class to the range.
    pub fn add_class(&mut self, class_idx: u8) {
        let word = (class_idx / 64) as usize;
        let bit = class_idx % 64;
        self.classes[word] |= 1u64 << bit;
    }

    /// Remove a hand class from the range.
    pub fn remove_class(&mut self, class_idx: u8) {
        let word = (class_idx / 64) as usize;
        let bit = class_idx % 64;
        self.classes[word] &= !(1u64 << bit);
    }

    /// Check if a hand class is in the range.
    pub fn contains_class(&self, class_idx: u8) -> bool {
        let word = (class_idx / 64) as usize;
        let bit = class_idx % 64;
        (self.classes[word] & (1u64 << bit)) != 0
    }

    /// Check if hole cards are in this range.
    pub fn contains(&self, hole_cards: &HoleCards) -> bool {
        self.contains_class(hole_cards.hand_class_index())
    }

    /// Count the number of hand classes in the range.
    pub fn num_classes(&self) -> usize {
        self.classes.iter().map(|w| w.count_ones() as usize).sum()
    }

    /// Count the total number of combos in the range.
    pub fn num_combos(&self) -> usize {
        self.iter_classes()
            .map(|hc| hc.num_combos() as usize)
            .sum()
    }

    /// Count combos not blocked by given cards.
    pub fn count_unblocked_combos(&self, blockers: &[Card]) -> usize {
        self.iter_classes()
            .map(|hc| hc.count_unblocked_combos(blockers) as usize)
            .sum()
    }

    /// Iterate over hand classes in the range.
    pub fn iter_classes(&self) -> impl Iterator<Item = HandClass> + '_ {
        (0..169u8).filter(move |&i| self.contains_class(i))
            .map(HandClass::from_index)
    }

    /// Enumerate all specific combos in the range.
    pub fn enumerate_combos(&self) -> Vec<HoleCards> {
        self.iter_classes()
            .flat_map(|hc| hc.enumerate_combos())
            .collect()
    }

    /// Enumerate combos not blocked by given cards.
    pub fn enumerate_unblocked_combos(&self, blockers: &[Card]) -> Vec<HoleCards> {
        let blocker_set: HashSet<u8> = blockers.iter().map(|c| c.id()).collect();
        self.enumerate_combos()
            .into_iter()
            .filter(|hc| {
                !blocker_set.contains(&hc.card1.id()) &&
                !blocker_set.contains(&hc.card2.id())
            })
            .collect()
    }

    /// Parse a range from notation string.
    /// Supports: "AA", "AKs", "AKo", "AK" (both suited and offsuit), "TT+", "AQs+", "A5s-A2s"
    pub fn from_notation(notation: &str) -> Result<Self, RangeParseError> {
        let mut range = Self::empty();

        for part in notation.split(',').map(|s| s.trim()) {
            if part.is_empty() {
                continue;
            }

            if let Err(e) = Self::parse_part(&mut range, part) {
                return Err(e);
            }
        }

        Ok(range)
    }

    /// Parse a single part of range notation.
    fn parse_part(range: &mut Range, part: &str) -> Result<(), RangeParseError> {
        let part = part.trim();

        // Check for range notation (e.g., "A5s-A2s")
        if part.contains('-') && !part.starts_with('-') {
            let parts: Vec<&str> = part.split('-').collect();
            if parts.len() == 2 {
                return Self::parse_range_notation(range, parts[0], parts[1]);
            }
        }

        // Check for plus notation (e.g., "TT+", "AQs+")
        if part.ends_with('+') {
            return Self::parse_plus_notation(range, &part[..part.len()-1]);
        }

        // Single hand class
        Self::parse_single_hand(range, part)
    }

    /// Parse a single hand notation like "AA", "AKs", "AKo", "AK".
    fn parse_single_hand(range: &mut Range, hand: &str) -> Result<(), RangeParseError> {
        let chars: Vec<char> = hand.chars().collect();

        if chars.len() < 2 || chars.len() > 3 {
            return Err(RangeParseError::InvalidFormat(hand.to_string()));
        }

        let r1 = Self::parse_rank(chars[0])?;
        let r2 = Self::parse_rank(chars[1])?;

        let (high, low) = if r1 >= r2 { (r1, r2) } else { (r2, r1) };

        if high == low {
            // Pair
            let hc = HandClass { rank1: high, rank2: low, suited: false };
            range.add_class(hc.index());
        } else if chars.len() == 3 {
            // Explicit suited/offsuit
            match chars[2] {
                's' | 'S' => {
                    let hc = HandClass { rank1: high, rank2: low, suited: true };
                    range.add_class(hc.index());
                }
                'o' | 'O' => {
                    let hc = HandClass { rank1: high, rank2: low, suited: false };
                    range.add_class(hc.index());
                }
                _ => return Err(RangeParseError::InvalidSuffix(chars[2])),
            }
        } else {
            // No suffix: add both suited and offsuit
            range.add_class(HandClass { rank1: high, rank2: low, suited: true }.index());
            range.add_class(HandClass { rank1: high, rank2: low, suited: false }.index());
        }

        Ok(())
    }

    /// Parse plus notation like "TT+" or "AQs+".
    fn parse_plus_notation(range: &mut Range, hand: &str) -> Result<(), RangeParseError> {
        let chars: Vec<char> = hand.chars().collect();

        if chars.len() < 2 {
            return Err(RangeParseError::InvalidFormat(hand.to_string()));
        }

        let r1 = Self::parse_rank(chars[0])?;
        let r2 = Self::parse_rank(chars[1])?;

        if r1 == r2 {
            // Pairs: TT+ means TT, JJ, QQ, KK, AA
            for rank in r1..13 {
                let hc = HandClass { rank1: rank, rank2: rank, suited: false };
                range.add_class(hc.index());
            }
        } else {
            // Non-pairs: AQs+ means AQs, AKs
            let (high, low) = if r1 >= r2 { (r1, r2) } else { (r2, r1) };
            let suited = chars.len() == 3 && (chars[2] == 's' || chars[2] == 'S');
            let offsuit = chars.len() == 3 && (chars[2] == 'o' || chars[2] == 'O');
            let both = chars.len() == 2;

            for kicker in low..high {
                if suited || both {
                    range.add_class(HandClass { rank1: high, rank2: kicker, suited: true }.index());
                }
                if offsuit || both {
                    range.add_class(HandClass { rank1: high, rank2: kicker, suited: false }.index());
                }
            }
        }

        Ok(())
    }

    /// Parse range notation like "A5s-A2s".
    fn parse_range_notation(range: &mut Range, start: &str, end: &str) -> Result<(), RangeParseError> {
        let start_chars: Vec<char> = start.chars().collect();
        let end_chars: Vec<char> = end.chars().collect();

        if start_chars.len() < 2 || end_chars.len() < 2 {
            return Err(RangeParseError::InvalidFormat(format!("{}-{}", start, end)));
        }

        let s_r1 = Self::parse_rank(start_chars[0])?;
        let s_r2 = Self::parse_rank(start_chars[1])?;
        let e_r1 = Self::parse_rank(end_chars[0])?;
        let e_r2 = Self::parse_rank(end_chars[1])?;

        // Must have same high card for range
        if s_r1 != e_r1 && s_r1.max(s_r2) != e_r1.max(e_r2) {
            return Err(RangeParseError::InvalidRange(format!("{}-{}", start, end)));
        }

        let high = s_r1.max(s_r2).max(e_r1).max(e_r2);
        let low_start = s_r1.min(s_r2);
        let low_end = e_r1.min(e_r2);

        let (low_min, low_max) = if low_start <= low_end {
            (low_start, low_end)
        } else {
            (low_end, low_start)
        };

        let suited = (start_chars.len() == 3 && (start_chars[2] == 's' || start_chars[2] == 'S'))
            || (end_chars.len() == 3 && (end_chars[2] == 's' || end_chars[2] == 'S'));
        let offsuit = (start_chars.len() == 3 && (start_chars[2] == 'o' || start_chars[2] == 'O'))
            || (end_chars.len() == 3 && (end_chars[2] == 'o' || end_chars[2] == 'O'));
        let both = start_chars.len() == 2 && end_chars.len() == 2;

        for low in low_min..=low_max {
            if suited || both {
                range.add_class(HandClass { rank1: high, rank2: low, suited: true }.index());
            }
            if offsuit || both {
                range.add_class(HandClass { rank1: high, rank2: low, suited: false }.index());
            }
        }

        Ok(())
    }

    /// Parse a single rank character.
    fn parse_rank(c: char) -> Result<u8, RangeParseError> {
        match c {
            '2' => Ok(0),
            '3' => Ok(1),
            '4' => Ok(2),
            '5' => Ok(3),
            '6' => Ok(4),
            '7' => Ok(5),
            '8' => Ok(6),
            '9' => Ok(7),
            'T' | 't' => Ok(8),
            'J' | 'j' => Ok(9),
            'Q' | 'q' => Ok(10),
            'K' | 'k' => Ok(11),
            'A' | 'a' => Ok(12),
            _ => Err(RangeParseError::InvalidRank(c)),
        }
    }
}

/// Error type for range parsing.
#[derive(Debug, Clone)]
pub enum RangeParseError {
    InvalidFormat(String),
    InvalidRank(char),
    InvalidSuffix(char),
    InvalidRange(String),
}

impl std::fmt::Display for RangeParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidFormat(s) => write!(f, "Invalid hand format: {}", s),
            Self::InvalidRank(c) => write!(f, "Invalid rank character: {}", c),
            Self::InvalidSuffix(c) => write!(f, "Invalid suffix: {} (expected 's' or 'o')", c),
            Self::InvalidRange(s) => write!(f, "Invalid range notation: {}", s),
        }
    }
}

impl std::error::Error for RangeParseError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_and_all() {
        let empty = Range::empty();
        assert_eq!(empty.num_classes(), 0);
        assert_eq!(empty.num_combos(), 0);

        let all = Range::all();
        assert_eq!(all.num_classes(), 169);
        assert_eq!(all.num_combos(), 1326);
    }

    #[test]
    fn test_parse_pairs() {
        let range = Range::from_notation("AA").unwrap();
        assert_eq!(range.num_classes(), 1);
        assert_eq!(range.num_combos(), 6);

        let range = Range::from_notation("AA, KK, QQ").unwrap();
        assert_eq!(range.num_classes(), 3);
        assert_eq!(range.num_combos(), 18);
    }

    #[test]
    fn test_parse_suited_offsuit() {
        let suited = Range::from_notation("AKs").unwrap();
        assert_eq!(suited.num_classes(), 1);
        assert_eq!(suited.num_combos(), 4);

        let offsuit = Range::from_notation("AKo").unwrap();
        assert_eq!(offsuit.num_classes(), 1);
        assert_eq!(offsuit.num_combos(), 12);

        let both = Range::from_notation("AK").unwrap();
        assert_eq!(both.num_classes(), 2);
        assert_eq!(both.num_combos(), 16);
    }

    #[test]
    fn test_parse_plus_notation() {
        // TT+ should include TT, JJ, QQ, KK, AA
        let range = Range::from_notation("TT+").unwrap();
        assert_eq!(range.num_classes(), 5);
        assert_eq!(range.num_combos(), 30);

        // AQs+ should include AQs, AKs
        let range = Range::from_notation("AQs+").unwrap();
        assert_eq!(range.num_classes(), 2);
        assert_eq!(range.num_combos(), 8);
    }

    #[test]
    fn test_parse_range_notation() {
        // A5s-A2s should include A2s, A3s, A4s, A5s
        let range = Range::from_notation("A5s-A2s").unwrap();
        assert_eq!(range.num_classes(), 4);
        assert_eq!(range.num_combos(), 16);
    }

    #[test]
    fn test_contains_hole_cards() {
        let range = Range::from_notation("AA, KK, AKs").unwrap();

        let aa = HoleCards::from_str("AhAs").unwrap();
        assert!(range.contains(&aa));

        let aks = HoleCards::from_str("AhKh").unwrap();
        assert!(range.contains(&aks));

        let ako = HoleCards::from_str("AhKd").unwrap();
        assert!(!range.contains(&ako));

        let qq = HoleCards::from_str("QhQs").unwrap();
        assert!(!range.contains(&qq));
    }

    #[test]
    fn test_enumerate_combos() {
        let range = Range::from_notation("AA").unwrap();
        let combos = range.enumerate_combos();
        assert_eq!(combos.len(), 6);

        // All combos should be AA
        for combo in &combos {
            assert!(combo.is_pair());
            assert_eq!(combo.card1.rank(), 12); // Ace
        }
    }

    #[test]
    fn test_enumerate_unblocked() {
        let range = Range::from_notation("AA").unwrap();

        // Block one ace
        let blockers = vec![Card::from_str("Ah").unwrap()];
        let combos = range.enumerate_unblocked_combos(&blockers);
        // With one ace blocked, only 3 combos remain
        assert_eq!(combos.len(), 3);
    }
}
