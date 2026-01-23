//! Preflop actions for 8-max solver.

use crate::cfr::game::Action;
use std::fmt;

/// Preflop betting action.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PreflopAction {
    /// Fold the hand.
    Fold,
    /// Call the current bet.
    Call,
    /// Raise to a specific amount (in centi-BB).
    Raise(u32),
    /// Go all-in.
    AllIn,
}

impl PreflopAction {
    /// Get a short code for this action.
    pub fn short_code(&self) -> String {
        match self {
            PreflopAction::Fold => "F".to_string(),
            PreflopAction::Call => "C".to_string(),
            PreflopAction::Raise(amt) => format!("R{}", amt),
            PreflopAction::AllIn => "A".to_string(),
        }
    }

    /// Check if this is an aggressive action.
    pub fn is_aggressive(&self) -> bool {
        matches!(self, PreflopAction::Raise(_) | PreflopAction::AllIn)
    }

    /// Get the raise amount if this is a raise.
    pub fn raise_amount(&self) -> Option<f64> {
        match self {
            PreflopAction::Raise(amt) => Some(*amt as f64 / 100.0),
            _ => None,
        }
    }
}

impl Action for PreflopAction {
    fn to_string(&self) -> String {
        self.short_code()
    }
}

impl fmt::Display for PreflopAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PreflopAction::Fold => write!(f, "Fold"),
            PreflopAction::Call => write!(f, "Call"),
            PreflopAction::Raise(amt) => write!(f, "Raise to {:.2}bb", *amt as f64 / 100.0),
            PreflopAction::AllIn => write!(f, "All-In"),
        }
    }
}

/// Convert BB to centi-BB.
#[inline]
pub fn bb_to_centi(bb: f64) -> u32 {
    (bb * 100.0).round() as u32
}

/// Convert centi-BB to BB.
#[inline]
pub fn centi_to_bb(centi: u32) -> f64 {
    centi as f64 / 100.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_action_short_codes() {
        assert_eq!(PreflopAction::Fold.short_code(), "F");
        assert_eq!(PreflopAction::Call.short_code(), "C");
        assert_eq!(PreflopAction::AllIn.short_code(), "A");
        assert_eq!(PreflopAction::Raise(230).short_code(), "R230");
    }

    #[test]
    fn test_aggressive_actions() {
        assert!(!PreflopAction::Fold.is_aggressive());
        assert!(!PreflopAction::Call.is_aggressive());
        assert!(PreflopAction::Raise(300).is_aggressive());
        assert!(PreflopAction::AllIn.is_aggressive());
    }
}
