//! Card abstraction for information state bucketing.
//!
//! This module provides card abstraction to reduce the state space of the poker game.
//! - Preflop: 169 hand classes (direct mapping)
//! - Postflop: Equity-based bucketing into configurable number of buckets

use super::card::{HoleCards, Board, Street};
use super::hand_eval::calculate_equity_vs_random;

/// Configuration for card abstraction.
#[derive(Debug, Clone)]
pub struct AbstractionConfig {
    /// Number of buckets for flop (default: 1024)
    pub flop_buckets: u16,
    /// Number of buckets for turn (default: 256)
    pub turn_buckets: u16,
    /// Number of buckets for river (default: 256)
    pub river_buckets: u16,
    /// Number of samples for equity calculation
    pub equity_samples: usize,
}

impl Default for AbstractionConfig {
    fn default() -> Self {
        Self {
            flop_buckets: 1024,
            turn_buckets: 256,
            river_buckets: 256,
            equity_samples: 500,
        }
    }
}

impl AbstractionConfig {
    /// Create a fast configuration with fewer buckets for testing.
    pub fn fast() -> Self {
        Self {
            flop_buckets: 100,
            turn_buckets: 50,
            river_buckets: 50,
            equity_samples: 100,
        }
    }

    /// Create a high-precision configuration.
    pub fn high_precision() -> Self {
        Self {
            flop_buckets: 2048,
            turn_buckets: 512,
            river_buckets: 512,
            equity_samples: 1000,
        }
    }
}

/// Card abstraction system for poker.
#[derive(Debug, Clone)]
pub struct CardAbstraction {
    config: AbstractionConfig,
}

impl CardAbstraction {
    /// Create a new card abstraction with default configuration.
    pub fn new() -> Self {
        Self {
            config: AbstractionConfig::default(),
        }
    }

    /// Create with custom configuration.
    pub fn with_config(config: AbstractionConfig) -> Self {
        Self { config }
    }

    /// Get the abstracted bucket for a hand on a given street.
    pub fn get_bucket(&self, hole_cards: &HoleCards, board: &Board) -> u16 {
        match board.street() {
            Street::Preflop => self.preflop_bucket(hole_cards),
            Street::Flop => self.postflop_bucket(hole_cards, board, self.config.flop_buckets),
            Street::Turn => self.postflop_bucket(hole_cards, board, self.config.turn_buckets),
            Street::River | Street::Showdown => self.postflop_bucket(hole_cards, board, self.config.river_buckets),
        }
    }

    /// Get preflop bucket (0-168) based on hand class.
    #[inline]
    pub fn preflop_bucket(&self, hole_cards: &HoleCards) -> u16 {
        hole_cards.hand_class_index() as u16
    }

    /// Get postflop bucket based on equity vs random hands.
    fn postflop_bucket(&self, hole_cards: &HoleCards, board: &Board, num_buckets: u16) -> u16 {
        let equity = calculate_equity_vs_random(hole_cards, board, self.config.equity_samples);
        // Map equity [0, 1] to bucket [0, num_buckets-1]
        let bucket = (equity * num_buckets as f64).floor() as u16;
        bucket.min(num_buckets - 1)
    }

    /// Get the number of buckets for a street.
    pub fn num_buckets(&self, street: Street) -> u16 {
        match street {
            Street::Preflop => 169,
            Street::Flop => self.config.flop_buckets,
            Street::Turn => self.config.turn_buckets,
            Street::River | Street::Showdown => self.config.river_buckets,
        }
    }

    /// Generate a bucket key for information state.
    pub fn bucket_key(&self, hole_cards: &HoleCards, board: &Board) -> String {
        let bucket = self.get_bucket(hole_cards, board);
        let street = board.street();
        format!("S{}B{}", street.index(), bucket)
    }
}

impl Default for CardAbstraction {
    fn default() -> Self {
        Self::new()
    }
}

/// Precomputed hand class information.
/// There are 169 strategically distinct preflop hands:
/// - 13 pairs (AA, KK, ..., 22)
/// - 78 suited hands (AKs, AQs, ..., 32s)
/// - 78 offsuit hands (AKo, AQo, ..., 32o)
#[derive(Debug, Clone)]
pub struct HandClass {
    /// Rank of first card (higher)
    pub rank1: u8,
    /// Rank of second card (lower or equal)
    pub rank2: u8,
    /// Whether suited
    pub suited: bool,
}

impl HandClass {
    /// Get hand class from index (0-168).
    pub fn from_index(index: u8) -> Self {
        if index < 13 {
            // Pairs
            Self {
                rank1: index,
                rank2: index,
                suited: false, // pairs are inherently not suited
            }
        } else if index < 91 {
            // Suited hands (13-90)
            let idx = index - 13;
            // Decode from triangular number
            let (r1, r2) = decode_triangular(idx);
            Self {
                rank1: r1,
                rank2: r2,
                suited: true,
            }
        } else {
            // Offsuit hands (91-168)
            let idx = index - 91;
            let (r1, r2) = decode_triangular(idx);
            Self {
                rank1: r1,
                rank2: r2,
                suited: false,
            }
        }
    }

    /// Get the hand class index (0-168).
    pub fn index(&self) -> u8 {
        if self.rank1 == self.rank2 {
            self.rank1
        } else if self.suited {
            13 + encode_triangular(self.rank1, self.rank2)
        } else {
            91 + encode_triangular(self.rank1, self.rank2)
        }
    }

    /// Get display string (e.g., "AKs", "QQ", "72o").
    pub fn to_string(&self) -> String {
        const RANK_CHARS: [char; 13] = ['2', '3', '4', '5', '6', '7', '8', '9', 'T', 'J', 'Q', 'K', 'A'];

        if self.rank1 == self.rank2 {
            format!("{}{}", RANK_CHARS[self.rank1 as usize], RANK_CHARS[self.rank2 as usize])
        } else {
            let suffix = if self.suited { 's' } else { 'o' };
            format!("{}{}{}", RANK_CHARS[self.rank1 as usize], RANK_CHARS[self.rank2 as usize], suffix)
        }
    }

    /// Number of combinations for this hand class.
    pub fn num_combos(&self) -> u8 {
        if self.rank1 == self.rank2 {
            6 // C(4,2) = 6 pair combos
        } else if self.suited {
            4 // 4 suited combos
        } else {
            12 // 12 offsuit combos
        }
    }

    /// Enumerate all specific combos for this hand class.
    pub fn enumerate_combos(&self) -> Vec<HoleCards> {
        use super::card::Card;

        let mut combos = Vec::with_capacity(self.num_combos() as usize);

        if self.rank1 == self.rank2 {
            // Pairs: all combinations of suits
            for s1 in 0..4u8 {
                for s2 in (s1+1)..4u8 {
                    let c1 = Card::new(self.rank1, s1);
                    let c2 = Card::new(self.rank2, s2);
                    combos.push(HoleCards::new(c1, c2));
                }
            }
        } else if self.suited {
            // Suited: same suit for both cards
            for suit in 0..4u8 {
                let c1 = Card::new(self.rank1, suit);
                let c2 = Card::new(self.rank2, suit);
                combos.push(HoleCards::new(c1, c2));
            }
        } else {
            // Offsuit: different suits
            for s1 in 0..4u8 {
                for s2 in 0..4u8 {
                    if s1 != s2 {
                        let c1 = Card::new(self.rank1, s1);
                        let c2 = Card::new(self.rank2, s2);
                        combos.push(HoleCards::new(c1, c2));
                    }
                }
            }
        }

        combos
    }

    /// Count combinations not blocked by given cards.
    pub fn count_unblocked_combos(&self, blockers: &[super::card::Card]) -> u8 {
        let combos = self.enumerate_combos();
        combos.iter()
            .filter(|hc| !blockers.iter().any(|b| hc.contains(*b)))
            .count() as u8
    }
}

/// Encode two ranks (r1 > r2) to triangular index.
fn encode_triangular(r1: u8, r2: u8) -> u8 {
    debug_assert!(r1 > r2);
    (r1 * (r1 - 1) / 2 + r2) as u8
}

/// Decode triangular index to two ranks (r1 > r2).
fn decode_triangular(idx: u8) -> (u8, u8) {
    // r1 is the row, r2 is the column in the triangular matrix
    let mut r1 = 1u8;
    let mut remaining = idx;

    while remaining >= r1 {
        remaining -= r1;
        r1 += 1;
    }

    (r1, remaining)
}

/// Iterator over all 169 hand classes.
pub struct HandClassIter {
    index: u8,
}

impl HandClassIter {
    pub fn new() -> Self {
        Self { index: 0 }
    }
}

impl Iterator for HandClassIter {
    type Item = HandClass;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index < 169 {
            let hc = HandClass::from_index(self.index);
            self.index += 1;
            Some(hc)
        } else {
            None
        }
    }
}

impl Default for HandClassIter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hand_class_pairs() {
        // Check all pairs have correct index
        for rank in 0..13u8 {
            let hc = HandClass::from_index(rank);
            assert_eq!(hc.rank1, rank);
            assert_eq!(hc.rank2, rank);
            assert!(!hc.suited);
            assert_eq!(hc.index(), rank);
            assert_eq!(hc.num_combos(), 6);
        }

        // AA should be index 12
        let aa = HandClass::from_index(12);
        assert_eq!(aa.to_string(), "AA");

        // 22 should be index 0
        let twos = HandClass::from_index(0);
        assert_eq!(twos.to_string(), "22");
    }

    #[test]
    fn test_hand_class_suited() {
        // First suited hand (index 13) should be 32s
        let first_suited = HandClass::from_index(13);
        assert!(first_suited.suited);
        assert_eq!(first_suited.num_combos(), 4);

        // Test roundtrip for all suited hands
        for idx in 13..91u8 {
            let hc = HandClass::from_index(idx);
            assert!(hc.suited);
            assert!(hc.rank1 > hc.rank2, "rank1 should be > rank2 for suited");
            assert_eq!(hc.index(), idx, "roundtrip failed for index {}", idx);
        }
    }

    #[test]
    fn test_hand_class_offsuit() {
        // Test roundtrip for all offsuit hands
        for idx in 91..169u8 {
            let hc = HandClass::from_index(idx);
            assert!(!hc.suited);
            assert!(hc.rank1 > hc.rank2, "rank1 should be > rank2 for offsuit");
            assert_eq!(hc.index(), idx, "roundtrip failed for index {}", idx);
            assert_eq!(hc.num_combos(), 12);
        }
    }

    #[test]
    fn test_enumerate_combos() {
        // Pairs should have 6 combos
        let aa = HandClass::from_index(12);
        assert_eq!(aa.enumerate_combos().len(), 6);

        // Suited should have 4 combos
        let aks = HandClass { rank1: 12, rank2: 11, suited: true };
        let combos = aks.enumerate_combos();
        assert_eq!(combos.len(), 4);
        for combo in &combos {
            assert!(combo.is_suited());
        }

        // Offsuit should have 12 combos
        let ako = HandClass { rank1: 12, rank2: 11, suited: false };
        let combos = ako.enumerate_combos();
        assert_eq!(combos.len(), 12);
        for combo in &combos {
            assert!(!combo.is_suited());
        }
    }

    #[test]
    fn test_hand_class_iterator() {
        let classes: Vec<_> = HandClassIter::new().collect();
        assert_eq!(classes.len(), 169);

        // Count combos should total 1326 (52 choose 2)
        let total_combos: u32 = classes.iter()
            .map(|hc| hc.num_combos() as u32)
            .sum();
        assert_eq!(total_combos, 1326);
    }

    #[test]
    fn test_abstraction_preflop() {
        let abstraction = CardAbstraction::new();
        let board = Board::new();

        // Test AA
        let aa = HoleCards::from_str("AhAs").unwrap();
        let bucket = abstraction.get_bucket(&aa, &board);
        assert_eq!(bucket, 12); // AA is hand class 12

        // Test 22
        let twos = HoleCards::from_str("2h2s").unwrap();
        let bucket = abstraction.get_bucket(&twos, &board);
        assert_eq!(bucket, 0); // 22 is hand class 0
    }

    #[test]
    fn test_abstraction_postflop() {
        let abstraction = CardAbstraction::with_config(AbstractionConfig::fast());

        // On flop, bucket should be based on equity
        let aa = HoleCards::from_str("AhAs").unwrap();
        let board = Board::from_str("Kd Qc 2s").unwrap();

        let bucket = abstraction.get_bucket(&aa, &board);
        // AA should have high equity, so high bucket
        assert!(bucket > abstraction.config.flop_buckets / 2,
            "AA bucket {} should be in upper half", bucket);
    }

    #[test]
    fn test_bucket_key_generation() {
        let abstraction = CardAbstraction::new();

        let aa = HoleCards::from_str("AhAs").unwrap();
        let preflop_board = Board::new();
        let key = abstraction.bucket_key(&aa, &preflop_board);
        assert!(key.starts_with("S0B"), "Preflop key should start with S0B, got {}", key);

        let flop_board = Board::from_str("Kd Qc 2s").unwrap();
        let key = abstraction.bucket_key(&aa, &flop_board);
        assert!(key.starts_with("S1B"), "Flop key should start with S1B, got {}", key);
    }
}
