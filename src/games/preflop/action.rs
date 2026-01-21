//! Poker actions for betting.
//!
//! This module defines the actions available in poker betting rounds.

use crate::cfr::game::Action;
use std::fmt;

/// A poker betting action.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PokerAction {
    /// Fold the hand, forfeiting any money invested.
    Fold,
    /// Check (pass action when no bet to call).
    Check,
    /// Call the current bet.
    Call,
    /// Bet a specific amount (in centi-BB, e.g., 100 = 1bb).
    /// This is used when there's no previous bet on the street.
    Bet(u32),
    /// Raise to a specific amount (in centi-BB).
    /// This is used when facing a bet.
    Raise(u32),
    /// Go all-in for all remaining chips.
    AllIn,
}

impl PokerAction {
    /// Check if this is a voluntary money-putting action.
    pub fn is_aggressive(&self) -> bool {
        matches!(self, PokerAction::Bet(_) | PokerAction::Raise(_) | PokerAction::AllIn)
    }

    /// Check if this is a fold.
    pub fn is_fold(&self) -> bool {
        matches!(self, PokerAction::Fold)
    }

    /// Check if this is a check.
    pub fn is_check(&self) -> bool {
        matches!(self, PokerAction::Check)
    }

    /// Check if this is a call.
    pub fn is_call(&self) -> bool {
        matches!(self, PokerAction::Call)
    }

    /// Check if this is all-in.
    pub fn is_allin(&self) -> bool {
        matches!(self, PokerAction::AllIn)
    }

    /// Get the bet/raise amount if applicable.
    pub fn amount(&self) -> Option<u32> {
        match self {
            PokerAction::Bet(amt) | PokerAction::Raise(amt) => Some(*amt),
            _ => None,
        }
    }

    /// Get a short code for this action (for info state keys).
    pub fn short_code(&self) -> String {
        match self {
            PokerAction::Fold => "F".to_string(),
            PokerAction::Check => "X".to_string(),
            PokerAction::Call => "C".to_string(),
            PokerAction::Bet(amt) => format!("B{}", amt),
            PokerAction::Raise(amt) => format!("R{}", amt),
            PokerAction::AllIn => "A".to_string(),
        }
    }

    /// Parse an action from its short code.
    pub fn from_short_code(code: &str) -> Option<Self> {
        if code.is_empty() {
            return None;
        }

        match code.chars().next()? {
            'F' => Some(PokerAction::Fold),
            'X' => Some(PokerAction::Check),
            'C' => Some(PokerAction::Call),
            'A' => Some(PokerAction::AllIn),
            'B' => {
                let amt: u32 = code[1..].parse().ok()?;
                Some(PokerAction::Bet(amt))
            }
            'R' => {
                let amt: u32 = code[1..].parse().ok()?;
                Some(PokerAction::Raise(amt))
            }
            _ => None,
        }
    }
}

impl Action for PokerAction {
    fn to_string(&self) -> String {
        self.short_code()
    }
}

impl fmt::Display for PokerAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PokerAction::Fold => write!(f, "Fold"),
            PokerAction::Check => write!(f, "Check"),
            PokerAction::Call => write!(f, "Call"),
            PokerAction::Bet(amt) => write!(f, "Bet {:.2}bb", *amt as f64 / 100.0),
            PokerAction::Raise(amt) => write!(f, "Raise to {:.2}bb", *amt as f64 / 100.0),
            PokerAction::AllIn => write!(f, "All-In"),
        }
    }
}

/// Convert a bet size in BB to centi-BB (integer representation).
#[inline]
pub fn bb_to_centi(bb: f64) -> u32 {
    (bb * 100.0).round() as u32
}

/// Convert centi-BB to BB.
#[inline]
pub fn centi_to_bb(centi: u32) -> f64 {
    centi as f64 / 100.0
}

/// Action abstraction for reducing the action space.
/// Maps continuous bet sizes to discrete buckets.
#[derive(Debug, Clone)]
pub struct ActionAbstraction {
    /// Bet sizes as fractions of pot (e.g., [0.33, 0.66, 1.0, 1.5])
    pub bet_sizes: Vec<f64>,
    /// Raise sizes as fractions of pot
    pub raise_sizes: Vec<f64>,
    /// SPR threshold below which all-in is always an option
    pub allin_threshold_spr: f64,
    /// Allow geometric sizing
    pub use_geometric: bool,
}

impl Default for ActionAbstraction {
    fn default() -> Self {
        Self {
            bet_sizes: vec![0.66], // 66% pot default
            raise_sizes: vec![0.66],
            allin_threshold_spr: 5.0,
            use_geometric: true,
        }
    }
}

impl ActionAbstraction {
    /// Create with geometric 66% pot sizing.
    pub fn geometric_66() -> Self {
        Self {
            bet_sizes: vec![0.66],
            raise_sizes: vec![0.66],
            allin_threshold_spr: 5.0,
            use_geometric: true,
        }
    }

    /// Create with multiple bet sizes.
    pub fn multi_size(sizes: Vec<f64>) -> Self {
        Self {
            bet_sizes: sizes.clone(),
            raise_sizes: sizes,
            allin_threshold_spr: 5.0,
            use_geometric: false,
        }
    }

    /// Get available bet sizes for a given pot and stack.
    pub fn get_bet_sizes(&self, pot: f64, stack: f64, min_bet: f64) -> Vec<f64> {
        let mut sizes = Vec::new();
        let spr = stack / pot;

        for &frac in &self.bet_sizes {
            let bet = pot * frac;
            if bet >= min_bet && bet < stack {
                sizes.push(bet);
            }
        }

        // Add all-in if SPR is low enough or no other sizes available
        if spr <= self.allin_threshold_spr || sizes.is_empty() {
            // All-in is represented as the stack amount
            if !sizes.iter().any(|&s| (s - stack).abs() < 0.01) {
                sizes.push(stack);
            }
        }

        sizes.sort_by(|a, b| a.partial_cmp(b).unwrap());
        sizes
    }

    /// Get available raise sizes when facing a bet.
    pub fn get_raise_sizes(&self, pot: f64, stack: f64, facing_bet: f64, min_raise: f64) -> Vec<f64> {
        let mut sizes = Vec::new();
        let pot_after_call = pot + facing_bet;
        let spr = stack / pot_after_call;

        for &frac in &self.raise_sizes {
            // Raise = facing_bet + frac * (pot + facing_bet)
            let raise_to = facing_bet + pot_after_call * frac;
            if raise_to >= min_raise && raise_to < stack {
                sizes.push(raise_to);
            }
        }

        // Add all-in if SPR is low enough
        if spr <= self.allin_threshold_spr || sizes.is_empty() {
            if !sizes.iter().any(|&s| (s - stack).abs() < 0.01) {
                sizes.push(stack);
            }
        }

        sizes.sort_by(|a, b| a.partial_cmp(b).unwrap());
        sizes
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_action_short_codes() {
        assert_eq!(PokerAction::Fold.short_code(), "F");
        assert_eq!(PokerAction::Check.short_code(), "X");
        assert_eq!(PokerAction::Call.short_code(), "C");
        assert_eq!(PokerAction::AllIn.short_code(), "A");
        assert_eq!(PokerAction::Bet(150).short_code(), "B150");
        assert_eq!(PokerAction::Raise(450).short_code(), "R450");
    }

    #[test]
    fn test_action_parsing() {
        assert_eq!(PokerAction::from_short_code("F"), Some(PokerAction::Fold));
        assert_eq!(PokerAction::from_short_code("X"), Some(PokerAction::Check));
        assert_eq!(PokerAction::from_short_code("C"), Some(PokerAction::Call));
        assert_eq!(PokerAction::from_short_code("A"), Some(PokerAction::AllIn));
        assert_eq!(PokerAction::from_short_code("B150"), Some(PokerAction::Bet(150)));
        assert_eq!(PokerAction::from_short_code("R450"), Some(PokerAction::Raise(450)));
        assert_eq!(PokerAction::from_short_code(""), None);
    }

    #[test]
    fn test_bb_conversion() {
        assert_eq!(bb_to_centi(1.5), 150);
        assert_eq!(bb_to_centi(2.3), 230);
        assert_eq!(centi_to_bb(150), 1.5);
        assert_eq!(centi_to_bb(230), 2.3);
    }

    #[test]
    fn test_action_properties() {
        assert!(PokerAction::Bet(100).is_aggressive());
        assert!(PokerAction::Raise(200).is_aggressive());
        assert!(PokerAction::AllIn.is_aggressive());
        assert!(!PokerAction::Check.is_aggressive());
        assert!(!PokerAction::Call.is_aggressive());
        assert!(!PokerAction::Fold.is_aggressive());
    }

    #[test]
    fn test_action_abstraction_bet_sizes() {
        let abstraction = ActionAbstraction::geometric_66();

        // Pot = 10bb, stack = 100bb (high SPR, no all-in added)
        let sizes = abstraction.get_bet_sizes(10.0, 100.0, 1.0);
        assert_eq!(sizes.len(), 1);
        assert!((sizes[0] - 6.6).abs() < 0.1);

        // Pot = 10bb, stack = 50bb (SPR = 5.0, equals threshold, includes all-in)
        let sizes = abstraction.get_bet_sizes(10.0, 50.0, 1.0);
        assert_eq!(sizes.len(), 2);  // 6.6bb bet + 50bb all-in
        assert!((sizes[0] - 6.6).abs() < 0.1);
        assert!((sizes[1] - 50.0).abs() < 0.01);

        // Pot = 10bb, stack = 4bb (low SPR, should include all-in)
        let sizes = abstraction.get_bet_sizes(10.0, 4.0, 1.0);
        assert!(sizes.iter().any(|&s| (s - 4.0).abs() < 0.01));
    }

    #[test]
    fn test_action_abstraction_raise_sizes() {
        let abstraction = ActionAbstraction::geometric_66();

        // Pot = 10bb, stack = 50bb, facing bet of 6.6bb
        let sizes = abstraction.get_raise_sizes(10.0, 50.0, 6.6, 13.2);
        // Raise should be 6.6 + 0.66 * (10 + 6.6) = 6.6 + 10.956 = 17.556
        assert!(!sizes.is_empty());
    }
}
