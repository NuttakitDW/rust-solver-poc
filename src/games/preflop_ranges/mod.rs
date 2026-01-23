//! Preflop Range Solver
//!
//! This module implements a preflop-only poker solver that outputs
//! human-readable range charts organized by position and scenario.
//!
//! Output format:
//! - Organized by scenario (SB_RFI, BB_vs_SB, etc.)
//! - Each hand shown as AA, AKs, AKo, etc.
//! - Action frequencies: fold%, call%, raise%

mod state;
mod game;
mod output;

pub use state::{PreflopRangeState, Position, Scenario, ActionType};
pub use game::{PreflopRangeGame, PreflopRangeConfig, solve_scenario};
pub use output::{RangeOutput, ScenarioRange, HandStrategy, generate_html};

/// Hand names in standard notation (13x13 grid order)
pub const HAND_NAMES: [&str; 169] = [
    // Row 0: Ax hands (AA at top-left)
    "AA", "AKs", "AQs", "AJs", "ATs", "A9s", "A8s", "A7s", "A6s", "A5s", "A4s", "A3s", "A2s",
    // Row 1: Kx hands
    "AKo", "KK", "KQs", "KJs", "KTs", "K9s", "K8s", "K7s", "K6s", "K5s", "K4s", "K3s", "K2s",
    // Row 2: Qx hands
    "AQo", "KQo", "QQ", "QJs", "QTs", "Q9s", "Q8s", "Q7s", "Q6s", "Q5s", "Q4s", "Q3s", "Q2s",
    // Row 3: Jx hands
    "AJo", "KJo", "QJo", "JJ", "JTs", "J9s", "J8s", "J7s", "J6s", "J5s", "J4s", "J3s", "J2s",
    // Row 4: Tx hands
    "ATo", "KTo", "QTo", "JTo", "TT", "T9s", "T8s", "T7s", "T6s", "T5s", "T4s", "T3s", "T2s",
    // Row 5: 9x hands
    "A9o", "K9o", "Q9o", "J9o", "T9o", "99", "98s", "97s", "96s", "95s", "94s", "93s", "92s",
    // Row 6: 8x hands
    "A8o", "K8o", "Q8o", "J8o", "T8o", "98o", "88", "87s", "86s", "85s", "84s", "83s", "82s",
    // Row 7: 7x hands
    "A7o", "K7o", "Q7o", "J7o", "T7o", "97o", "87o", "77", "76s", "75s", "74s", "73s", "72s",
    // Row 8: 6x hands
    "A6o", "K6o", "Q6o", "J6o", "T6o", "96o", "86o", "76o", "66", "65s", "64s", "63s", "62s",
    // Row 9: 5x hands
    "A5o", "K5o", "Q5o", "J5o", "T5o", "95o", "85o", "75o", "65o", "55", "54s", "53s", "52s",
    // Row 10: 4x hands
    "A4o", "K4o", "Q4o", "J4o", "T4o", "94o", "84o", "74o", "64o", "54o", "44", "43s", "42s",
    // Row 11: 3x hands
    "A3o", "K3o", "Q3o", "J3o", "T3o", "93o", "83o", "73o", "63o", "53o", "43o", "33", "32s",
    // Row 12: 2x hands
    "A2o", "K2o", "Q2o", "J2o", "T2o", "92o", "82o", "72o", "62o", "52o", "42o", "32o", "22",
];

/// Convert internal hand class index (0-168) to grid position (row, col)
pub fn hand_class_to_grid(class_idx: u8) -> (usize, usize) {
    // Internal order: pairs (0-12), suited (13-90), offsuit (91-168)
    // Grid order: row = high rank (A=0, K=1, ..., 2=12), col based on second card

    let (rank1, rank2, suited) = decode_hand_class(class_idx);

    // Convert to grid position
    // High card determines row (A=0, K=1, etc.)
    // For pairs: diagonal
    // For suited: above diagonal
    // For offsuit: below diagonal

    let high = 12 - rank1; // A=0, K=1, ..., 2=12
    let low = 12 - rank2;

    if rank1 == rank2 {
        // Pair - on diagonal
        (high as usize, high as usize)
    } else if suited {
        // Suited - above diagonal (row < col)
        (high as usize, low as usize)
    } else {
        // Offsuit - below diagonal (row > col)
        (low as usize, high as usize)
    }
}

/// Convert grid position to hand name
pub fn grid_to_hand_name(row: usize, col: usize) -> &'static str {
    HAND_NAMES[row * 13 + col]
}

/// Decode hand class index to (rank1, rank2, suited)
/// rank1 >= rank2, where A=12, K=11, ..., 2=0
fn decode_hand_class(class_idx: u8) -> (u8, u8, bool) {
    if class_idx < 13 {
        // Pairs: index 0-12 maps to 22-AA
        (class_idx, class_idx, false)
    } else if class_idx < 91 {
        // Suited: index 13-90
        let idx = class_idx - 13;
        let (r1, r2) = decode_triangular(idx);
        (r1, r2, true)
    } else {
        // Offsuit: index 91-168
        let idx = class_idx - 91;
        let (r1, r2) = decode_triangular(idx);
        (r1, r2, false)
    }
}

/// Decode triangular index to two ranks (r1 > r2)
fn decode_triangular(idx: u8) -> (u8, u8) {
    let mut r1 = 1u8;
    let mut remaining = idx;
    while remaining >= r1 {
        remaining -= r1;
        r1 += 1;
    }
    (r1, remaining)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hand_names_count() {
        assert_eq!(HAND_NAMES.len(), 169);
    }

    #[test]
    fn test_grid_positions() {
        // AA should be at (0, 0)
        assert_eq!(grid_to_hand_name(0, 0), "AA");
        // KK at (1, 1)
        assert_eq!(grid_to_hand_name(1, 1), "KK");
        // AKs at (0, 1)
        assert_eq!(grid_to_hand_name(0, 1), "AKs");
        // AKo at (1, 0)
        assert_eq!(grid_to_hand_name(1, 0), "AKo");
        // 22 at (12, 12)
        assert_eq!(grid_to_hand_name(12, 12), "22");
    }
}
