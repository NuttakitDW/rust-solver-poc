//! Equity calculation for preflop-only solver.
//!
//! This module provides equity calculations to estimate postflop value
//! without solving the full postflop game tree. This enables fast preflop
//! convergence while still accounting for postflop playability.

use crate::games::preflop::abstraction::HandClass;

/// Equity calculator for preflop hands.
#[derive(Debug, Clone)]
pub struct EquityCalculator {
    /// Precomputed equity matrix for hand class vs hand class.
    /// equity_matrix[i][j] = equity of hand class i vs hand class j.
    equity_matrix: Vec<Vec<f64>>,
    /// Whether the matrix has been initialized.
    initialized: bool,
}

impl EquityCalculator {
    /// Create a new equity calculator.
    pub fn new() -> Self {
        Self {
            equity_matrix: Vec::new(),
            initialized: false,
        }
    }

    /// Initialize with precomputed equities.
    /// For now, use approximate equities based on hand strength.
    pub fn initialize(&mut self) {
        if self.initialized {
            return;
        }

        // Create a 169x169 matrix
        self.equity_matrix = vec![vec![0.5; 169]; 169];

        // Compute approximate equities based on hand strength rankings
        // This is a simplified model - real equity requires Monte Carlo simulation
        for i in 0..169 {
            for j in 0..169 {
                let strength_i = hand_class_strength(i as u8);
                let strength_j = hand_class_strength(j as u8);

                // Simple model: equity based on relative strength
                // With some randomness to account for hand interactions
                let diff = strength_i - strength_j;
                let equity = 0.5 + 0.3 * diff.tanh();

                self.equity_matrix[i][j] = equity;
            }
        }

        self.initialized = true;
    }

    /// Get equity of hand class vs another hand class.
    pub fn equity_vs_hand(&self, our_class: u8, villain_class: u8) -> f64 {
        if !self.initialized {
            // Fallback to simple strength-based calculation
            let our_strength = hand_class_strength(our_class);
            let villain_strength = hand_class_strength(villain_class);
            let diff = our_strength - villain_strength;
            return 0.5 + 0.3 * diff.tanh();
        }

        self.equity_matrix[our_class as usize][villain_class as usize]
    }

    /// Get equity of hand class vs a range (weighted by combos).
    pub fn equity_vs_range(&self, our_class: u8, range_weights: &[f64; 169]) -> f64 {
        let our_hand = HandClass::from_index(our_class);
        let our_combos = our_hand.num_combos() as f64;

        let mut total_equity = 0.0;
        let mut total_weight = 0.0;

        for villain_class in 0..169u8 {
            let weight = range_weights[villain_class as usize];
            if weight < 0.0001 {
                continue;
            }

            let villain_hand = HandClass::from_index(villain_class);
            let villain_combos = villain_hand.num_combos() as f64;

            // Account for card removal (simplified)
            let removal_factor = if our_class == villain_class {
                // Same hand class - significant card removal
                0.0
            } else if shares_rank(&our_hand, &villain_hand) {
                // Shares a rank - some card removal
                0.8
            } else {
                1.0
            };

            let effective_combos = villain_combos * removal_factor * weight;
            let equity = self.equity_vs_hand(our_class, villain_class);

            total_equity += equity * effective_combos;
            total_weight += effective_combos;
        }

        if total_weight > 0.0 {
            total_equity / total_weight
        } else {
            0.5
        }
    }

    /// Estimate postflop EV given preflop equity and pot.
    /// Uses a simple model: EV = equity * pot * realization_factor
    pub fn estimate_postflop_ev(
        &self,
        equity: f64,
        pot: f64,
        invested: f64,
        is_ip: bool,
    ) -> f64 {
        // Equity realization factor
        // IP (in position) realizes more equity, OOP realizes less
        let realization = if is_ip { 1.05 } else { 0.95 };

        // Expected value: equity * pot * realization - invested
        (equity * pot * realization) - invested
    }
}

impl Default for EquityCalculator {
    fn default() -> Self {
        let mut calc = Self::new();
        calc.initialize();
        calc
    }
}

/// Get approximate strength score for a hand class (0.0 to 1.0).
/// Higher is better.
fn hand_class_strength(class_idx: u8) -> f64 {
    let hc = HandClass::from_index(class_idx);

    // Base strength from ranks (Ace = 12, King = 11, etc.)
    let high_rank = hc.rank1 as f64;
    let low_rank = hc.rank2 as f64;

    // Pair bonus
    let pair_bonus = if hc.rank1 == hc.rank2 {
        0.3 + (high_rank / 12.0) * 0.15
    } else {
        0.0
    };

    // Suited bonus
    let suited_bonus = if hc.suited { 0.05 } else { 0.0 };

    // Connectedness bonus (for straight potential)
    let gap = (hc.rank1 as i32 - hc.rank2 as i32).abs() as f64;
    let connected_bonus = if gap <= 3.0 { 0.02 * (4.0 - gap) } else { 0.0 };

    // High card bonus
    let high_card_bonus = (high_rank + low_rank * 0.5) / 24.0 * 0.4;

    // Combine into strength score
    let raw_strength = pair_bonus + suited_bonus + connected_bonus + high_card_bonus;

    // Normalize to 0-1 range
    (raw_strength * 2.0).min(1.0).max(0.0)
}

/// Check if two hand classes share a rank.
fn shares_rank(hc1: &HandClass, hc2: &HandClass) -> bool {
    hc1.rank1 == hc2.rank1 || hc1.rank1 == hc2.rank2 ||
    hc1.rank2 == hc2.rank1 || hc1.rank2 == hc2.rank2
}

/// Precomputed equity table for common preflop matchups.
/// This can be loaded from a file for more accurate values.
pub mod precomputed {
    /// Get equity for AA vs a random hand.
    pub const AA_VS_RANDOM: f64 = 0.8507;
    /// Get equity for KK vs a random hand.
    pub const KK_VS_RANDOM: f64 = 0.8227;
    /// Get equity for QQ vs a random hand.
    pub const QQ_VS_RANDOM: f64 = 0.7973;
    /// Get equity for 72o vs a random hand.
    pub const _72O_VS_RANDOM: f64 = 0.3457;

    /// Simple equity vs random lookup by hand class.
    pub fn equity_vs_random(class_idx: u8) -> f64 {
        // Approximate values - should be replaced with Monte Carlo results
        EQUITY_VS_RANDOM[class_idx as usize]
    }

    /// Approximate equity vs random for all 169 hand classes.
    /// Order: pairs (22-AA), suited (32s-AKs), offsuit (32o-AKo)
    pub const EQUITY_VS_RANDOM: [f64; 169] = [
        // Pairs: 22-AA
        0.5021, 0.5369, 0.5706, 0.6032, 0.6348, 0.6655, 0.6951, 0.7236,
        0.7510, 0.7773, 0.7973, 0.8227, 0.8507,
        // Suited hands (78 hands): 32s, 42s, 52s, ..., AKs
        0.3744, 0.3840, 0.3939, 0.4041, 0.4147, 0.4257, 0.4371, 0.4491, 0.4616, 0.4747, 0.4884, 0.5028,
        0.3909, 0.4010, 0.4114, 0.4222, 0.4335, 0.4453, 0.4577, 0.4707, 0.4844, 0.4988, 0.5140,
        0.4078, 0.4184, 0.4295, 0.4410, 0.4531, 0.4659, 0.4793, 0.4934, 0.5083, 0.5241,
        0.4249, 0.4359, 0.4474, 0.4595, 0.4722, 0.4856, 0.4998, 0.5148, 0.5307,
        0.4424, 0.4539, 0.4659, 0.4786, 0.4920, 0.5061, 0.5211, 0.5371,
        0.4604, 0.4724, 0.4851, 0.4984, 0.5126, 0.5276, 0.5436,
        0.4789, 0.4915, 0.5048, 0.5189, 0.5339, 0.5499,
        0.4981, 0.5113, 0.5253, 0.5402, 0.5562,
        0.5181, 0.5320, 0.5469, 0.5628,
        0.5391, 0.5538, 0.5697,
        0.5611, 0.5768,
        0.5844,
        // Offsuit hands (78 hands): 32o, 42o, 52o, ..., AKo
        0.3457, 0.3551, 0.3648, 0.3749, 0.3854, 0.3963, 0.4077, 0.4196, 0.4322, 0.4454, 0.4593, 0.4740,
        0.3617, 0.3716, 0.3818, 0.3925, 0.4037, 0.4154, 0.4277, 0.4407, 0.4544, 0.4689, 0.4843,
        0.3782, 0.3885, 0.3993, 0.4106, 0.4225, 0.4350, 0.4483, 0.4623, 0.4772, 0.4931,
        0.3949, 0.4057, 0.4170, 0.4289, 0.4414, 0.4547, 0.4688, 0.4838, 0.4998,
        0.4121, 0.4233, 0.4352, 0.4477, 0.4609, 0.4750, 0.4899, 0.5059,
        0.4297, 0.4415, 0.4540, 0.4672, 0.4812, 0.4961, 0.5121,
        0.4479, 0.4603, 0.4735, 0.4874, 0.5023, 0.5183,
        0.4668, 0.4799, 0.4938, 0.5086, 0.5245,
        0.4866, 0.5003, 0.5150, 0.5309,
        0.5073, 0.5219, 0.5377,
        0.5291, 0.5448,
        0.5522,
    ];
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hand_strength_ordering() {
        // AA (index 12) should be strongest pair
        let aa_strength = hand_class_strength(12);
        let kk_strength = hand_class_strength(11);
        let twos_strength = hand_class_strength(0);

        assert!(aa_strength > kk_strength);
        assert!(kk_strength > twos_strength);
    }

    #[test]
    fn test_equity_calculator() {
        let calc = EquityCalculator::default();

        // AA vs KK should favor AA
        let aa_vs_kk = calc.equity_vs_hand(12, 11);
        assert!(aa_vs_kk > 0.5);

        // KK vs AA should be inverse
        let kk_vs_aa = calc.equity_vs_hand(11, 12);
        assert!(kk_vs_aa < 0.5);
    }

    #[test]
    fn test_precomputed_equity() {
        use precomputed::*;

        assert!(AA_VS_RANDOM > 0.80);
        assert!(KK_VS_RANDOM > 0.80);
        assert!(_72O_VS_RANDOM < 0.40);
    }
}
