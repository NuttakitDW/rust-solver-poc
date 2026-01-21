//! Betting logic and action generation.
//!
//! This module handles the generation of available betting actions based on
//! game state, bet sizing configuration, and poker rules.

use super::action::{PokerAction, bb_to_centi};
use super::card::Street;
use super::state::PokerState;

/// Configuration for bet sizing.
#[derive(Debug, Clone)]
pub struct BettingConfig {
    /// Geometric bet size as fraction of pot (e.g., 0.66)
    pub geo_size: f64,
    /// SPR threshold below which all-in is always an option
    pub add_allin_spr: f64,
    /// Whether to allow donk bets (OOP betting into aggressor)
    pub allow_donk: bool,
    /// Maximum number of bets per street (-1 for unlimited)
    pub max_bets_per_street: i32,
    /// Preflop open raise sizes by situation
    pub preflop_open: PreflopOpenSizing,
    /// Preflop 3bet multipliers
    pub preflop_3bet: Preflop3BetSizing,
}

impl Default for BettingConfig {
    fn default() -> Self {
        Self {
            geo_size: 0.66,
            add_allin_spr: 5.0,
            allow_donk: false,
            max_bets_per_street: -1,
            preflop_open: PreflopOpenSizing::default(),
            preflop_3bet: Preflop3BetSizing::default(),
        }
    }
}

/// Preflop open raise sizing.
#[derive(Debug, Clone)]
pub struct PreflopOpenSizing {
    /// SB open raise size in BB
    pub sb_open: f64,
    /// Standard open raise size in BB
    pub standard_open: f64,
}

impl Default for PreflopOpenSizing {
    fn default() -> Self {
        Self {
            sb_open: 3.0,     // SB opens to 3bb (completing to 1bb + raising 2bb)
            standard_open: 2.5, // Standard open is 2.5bb
        }
    }
}

/// Preflop 3bet sizing.
#[derive(Debug, Clone)]
pub struct Preflop3BetSizing {
    /// Multiplier for IP 3bet (vs open)
    pub ip_multiplier: f64,
    /// Multiplier for OOP 3bet (vs open)
    pub oop_multiplier: f64,
    /// Multiplier for BB vs SB 3bet
    pub bb_vs_sb_multiplier: f64,
}

impl Default for Preflop3BetSizing {
    fn default() -> Self {
        Self {
            ip_multiplier: 2.5,
            oop_multiplier: 3.3,
            bb_vs_sb_multiplier: 2.5,
        }
    }
}

/// Betting logic handler.
#[derive(Debug, Clone)]
pub struct BettingLogic {
    config: BettingConfig,
}

impl BettingLogic {
    /// Create with default configuration.
    pub fn new() -> Self {
        Self {
            config: BettingConfig::default(),
        }
    }

    /// Create with custom configuration.
    pub fn with_config(config: BettingConfig) -> Self {
        Self { config }
    }

    /// Get the configuration.
    pub fn config(&self) -> &BettingConfig {
        &self.config
    }

    /// Get all available actions for the current state.
    pub fn available_actions(&self, state: &PokerState) -> Vec<PokerAction> {
        if state.is_terminal || state.to_act.is_none() {
            return vec![];
        }

        let pos = state.to_act.unwrap();
        let idx = pos.index();
        let stack = state.stacks[idx];
        let pot = state.pot;
        let to_call = state.to_call;

        let mut actions = Vec::new();

        // Always can fold if there's something to call
        if to_call > 0.0 {
            actions.push(PokerAction::Fold);
        }

        // Check if can check
        if to_call == 0.0 {
            actions.push(PokerAction::Check);
        }

        // Call if facing a bet
        if to_call > 0.0 && stack > 0.0 {
            actions.push(PokerAction::Call);
        }

        // Bet/Raise options
        if stack > 0.0 {
            let bet_actions = if to_call == 0.0 {
                self.get_bet_actions(state)
            } else {
                self.get_raise_actions(state)
            };
            actions.extend(bet_actions);
        }

        actions
    }

    /// Get available bet actions (when no bet to call).
    fn get_bet_actions(&self, state: &PokerState) -> Vec<PokerAction> {
        let pos = state.to_act.unwrap();
        let idx = pos.index();
        let stack = state.stacks[idx];
        let pot = state.pot;

        let mut actions = Vec::new();

        match state.street {
            Street::Preflop => {
                // Preflop open sizing
                let open_size = self.config.preflop_open.sb_open;
                if open_size < stack {
                    actions.push(PokerAction::Raise(bb_to_centi(open_size)));
                }
            }
            _ => {
                // Postflop: geometric sizing
                let bet_size = pot * self.config.geo_size;
                let min_bet = 1.0; // 1bb minimum

                if bet_size >= min_bet && bet_size < stack {
                    actions.push(PokerAction::Bet(bb_to_centi(bet_size)));
                }
            }
        }

        // Add all-in if SPR is low or stack is small
        let spr = stack / pot;
        if spr <= self.config.add_allin_spr || actions.is_empty() {
            actions.push(PokerAction::AllIn);
        } else if stack <= 1.0 {
            // Very small stack, just all-in
            actions.push(PokerAction::AllIn);
        }

        // Deduplicate (all-in might be same as a bet)
        self.deduplicate_actions(actions, stack)
    }

    /// Get available raise actions (when facing a bet).
    fn get_raise_actions(&self, state: &PokerState) -> Vec<PokerAction> {
        let pos = state.to_act.unwrap();
        let idx = pos.index();
        let stack = state.stacks[idx];
        let pot = state.pot;
        let to_call = state.to_call;
        let invested = state.invested_street[idx];
        let opp_invested = state.invested_street[pos.opponent().index()];

        // Check bet cap
        if self.config.max_bets_per_street > 0 &&
           state.num_bets_street >= self.config.max_bets_per_street as u8 {
            return vec![];
        }

        let mut actions = Vec::new();

        // Minimum raise is to match and then raise by at least the last bet size
        let min_raise_to = opp_invested + state.last_bet_size;
        let min_raise_amount = min_raise_to - invested;

        // Can only raise if we have enough chips
        if min_raise_amount > stack {
            return actions;
        }

        match state.street {
            Street::Preflop => {
                // 3bet sizing based on position
                // BB vs SB uses lower multiplier
                let multiplier = if state.num_bets_street == 1 {
                    // This is a 3bet
                    self.config.preflop_3bet.bb_vs_sb_multiplier
                } else {
                    // 4bet+
                    2.2 // Pot-ish sizing for 4bet+
                };

                let raise_to = opp_invested * multiplier;
                if raise_to >= min_raise_to && raise_to < stack + invested {
                    actions.push(PokerAction::Raise(bb_to_centi(raise_to)));
                }
            }
            _ => {
                // Postflop: geometric sizing for raises
                let pot_after_call = pot + to_call;
                let raise_size = pot_after_call * self.config.geo_size;
                let raise_to = opp_invested + raise_size;

                if raise_to >= min_raise_to && raise_to < stack + invested {
                    actions.push(PokerAction::Raise(bb_to_centi(raise_to)));
                }
            }
        }

        // Add all-in if SPR is low
        let spr = stack / pot;
        if spr <= self.config.add_allin_spr || actions.is_empty() {
            if stack > 0.0 {
                actions.push(PokerAction::AllIn);
            }
        }

        self.deduplicate_actions(actions, stack)
    }

    /// Remove duplicate actions (e.g., when all-in equals a normal bet).
    fn deduplicate_actions(&self, mut actions: Vec<PokerAction>, stack: f64) -> Vec<PokerAction> {
        let stack_centi = bb_to_centi(stack);

        // Check if any bet/raise is effectively all-in
        let has_allin = actions.iter().any(|a| matches!(a, PokerAction::AllIn));

        if has_allin {
            // Remove bets/raises that are >=95% of stack (effectively all-in)
            actions.retain(|a| {
                match a {
                    PokerAction::Bet(amt) | PokerAction::Raise(amt) => {
                        *amt < (stack_centi as f64 * 0.95) as u32
                    }
                    _ => true
                }
            });
        }

        actions
    }

    /// Calculate pot odds for a call.
    pub fn pot_odds(&self, state: &PokerState) -> f64 {
        if state.to_call == 0.0 {
            return 0.0;
        }
        state.to_call / (state.pot + state.to_call)
    }

    /// Calculate implied odds adjustment.
    pub fn implied_odds_factor(&self, state: &PokerState) -> f64 {
        let eff_stack = state.effective_stack();
        let pot = state.pot;

        // More room for implied odds with deeper stacks
        (eff_stack / pot).min(3.0)
    }
}

impl Default for BettingLogic {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::games::preflop::card::HoleCards;

    #[test]
    fn test_preflop_actions() {
        let betting = BettingLogic::new();
        let sb_hand = HoleCards::from_str("AsAd").unwrap();
        let bb_hand = HoleCards::from_str("KhKs").unwrap();

        let state = PokerState::new_hu([50.0, 50.0], 0.5, 1.0)
            .with_hands(sb_hand, bb_hand);

        let actions = betting.available_actions(&state);

        // SB should be able to fold, call (limp), raise, or all-in
        assert!(actions.contains(&PokerAction::Fold));
        assert!(actions.contains(&PokerAction::Call));
        assert!(actions.iter().any(|a| matches!(a, PokerAction::Raise(_))));
    }

    #[test]
    fn test_postflop_actions() {
        let betting = BettingLogic::new();
        let sb_hand = HoleCards::from_str("AsAd").unwrap();
        let bb_hand = HoleCards::from_str("KhKs").unwrap();

        let mut state = PokerState::new_hu([50.0, 50.0], 0.5, 1.0)
            .with_hands(sb_hand, bb_hand);

        // Go to flop
        state = state.apply(PokerAction::Call);
        state = state.apply(PokerAction::Check);

        // On flop, SB can check or bet
        let actions = betting.available_actions(&state);

        assert!(actions.contains(&PokerAction::Check));
        assert!(actions.iter().any(|a| matches!(a, PokerAction::Bet(_))));
        assert!(!actions.contains(&PokerAction::Fold)); // Can't fold when not facing bet
    }

    #[test]
    fn test_facing_bet_actions() {
        let betting = BettingLogic::new();
        let sb_hand = HoleCards::from_str("AsAd").unwrap();
        let bb_hand = HoleCards::from_str("KhKs").unwrap();

        let mut state = PokerState::new_hu([50.0, 50.0], 0.5, 1.0)
            .with_hands(sb_hand, bb_hand);

        // Go to flop
        state = state.apply(PokerAction::Call);
        state = state.apply(PokerAction::Check);

        // SB bets
        let bet_size = (state.pot * 0.66 * 100.0).round() as u32;
        state = state.apply(PokerAction::Bet(bet_size));

        // BB facing bet should be able to fold, call, or raise
        let actions = betting.available_actions(&state);

        assert!(actions.contains(&PokerAction::Fold));
        assert!(actions.contains(&PokerAction::Call));
        assert!(!actions.contains(&PokerAction::Check)); // Can't check when facing bet
    }

    #[test]
    fn test_all_in_threshold() {
        let config = BettingConfig {
            add_allin_spr: 5.0,
            ..Default::default()
        };
        let betting = BettingLogic::with_config(config);

        // Create state with low SPR
        let sb_hand = HoleCards::from_str("AsAd").unwrap();
        let bb_hand = HoleCards::from_str("KhKs").unwrap();

        let mut state = PokerState::new_hu([10.0, 10.0], 0.5, 1.0)
            .with_hands(sb_hand, bb_hand);

        // Go to flop with 6bb pot
        state = state.apply(PokerAction::Raise(300)); // SB raises to 3bb
        state = state.apply(PokerAction::Call);       // BB calls

        // SPR should be low, all-in should be an option
        let actions = betting.available_actions(&state);
        assert!(actions.iter().any(|a| matches!(a, PokerAction::AllIn)));
    }

    #[test]
    fn test_pot_odds() {
        let betting = BettingLogic::new();
        let sb_hand = HoleCards::from_str("AsAd").unwrap();
        let bb_hand = HoleCards::from_str("KhKs").unwrap();

        let mut state = PokerState::new_hu([50.0, 50.0], 0.5, 1.0)
            .with_hands(sb_hand, bb_hand);

        // SB raises to 3bb
        state = state.apply(PokerAction::Raise(300));

        // BB facing 2bb call into 4bb pot
        let odds = betting.pot_odds(&state);
        // to_call / (pot + to_call) = 2 / (3.5 + 2) = 2/5.5 ≈ 0.36
        assert!(odds > 0.3 && odds < 0.4, "Pot odds {} should be ~0.36", odds);
    }
}
