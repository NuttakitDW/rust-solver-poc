//! SB vs BB full game implementation.
//!
//! This module implements the Game trait for a complete heads-up poker game
//! between Small Blind and Big Blind, including all streets (preflop through river).

use rand::Rng;

use super::card::{HoleCards, Street};
use super::state::{PokerState, HUPosition};
use super::action::PokerAction;
use super::info_state::PokerInfoState;
use super::betting::{BettingLogic, BettingConfig};
use super::abstraction::{CardAbstraction, AbstractionConfig};
use super::hand_eval::HandEvaluator;
use crate::cfr::game::Game;

/// Configuration for the SB vs BB game.
#[derive(Debug, Clone)]
pub struct SBvsBBConfig {
    /// Starting stack in BB for both players
    pub stack_bb: f64,
    /// Small blind amount
    pub sb_amount: f64,
    /// Big blind amount
    pub bb_amount: f64,
    /// Betting configuration
    pub betting: BettingConfig,
    /// Card abstraction configuration
    pub abstraction: AbstractionConfig,
}

impl Default for SBvsBBConfig {
    fn default() -> Self {
        Self {
            stack_bb: 50.0,
            sb_amount: 0.5,
            bb_amount: 1.0,
            betting: BettingConfig::default(),
            abstraction: AbstractionConfig::default(),
        }
    }
}

impl SBvsBBConfig {
    /// Create a configuration for quick testing.
    pub fn fast() -> Self {
        Self {
            stack_bb: 50.0,
            sb_amount: 0.5,
            bb_amount: 1.0,
            betting: BettingConfig::default(),
            abstraction: AbstractionConfig::fast(),
        }
    }
}

/// SB vs BB heads-up poker game with full postflop play.
#[derive(Clone)]
pub struct SBvsBBFullGame {
    config: SBvsBBConfig,
    betting: BettingLogic,
    abstraction: CardAbstraction,
    evaluator: HandEvaluator,
}

impl SBvsBBFullGame {
    /// Create a new game with default configuration.
    pub fn new() -> Self {
        let config = SBvsBBConfig::default();
        Self::with_config(config)
    }

    /// Create a game with custom configuration.
    pub fn with_config(config: SBvsBBConfig) -> Self {
        let betting = BettingLogic::with_config(config.betting.clone());
        let abstraction = CardAbstraction::with_config(config.abstraction.clone());
        let evaluator = HandEvaluator::new();

        Self {
            config,
            betting,
            abstraction,
            evaluator,
        }
    }

    /// Create a fast testing configuration.
    pub fn fast() -> Self {
        Self::with_config(SBvsBBConfig::fast())
    }

    /// Get the game configuration.
    pub fn config(&self) -> &SBvsBBConfig {
        &self.config
    }

    /// Determine the winner at showdown.
    fn determine_showdown_winner(&self, state: &PokerState) -> Option<HUPosition> {
        let sb_hand = state.hand(HUPosition::SB)?;
        let bb_hand = state.hand(HUPosition::BB)?;

        let result = self.evaluator.compare(sb_hand, bb_hand, &state.board);

        if result > 0 {
            Some(HUPosition::SB)
        } else if result < 0 {
            Some(HUPosition::BB)
        } else {
            None // Tie
        }
    }

    /// Check if we need to deal cards (chance node).
    fn needs_deal(&self, state: &PokerState) -> bool {
        if state.is_terminal {
            return false;
        }

        // Need to deal hole cards
        if state.hands[0].is_none() || state.hands[1].is_none() {
            return true;
        }

        // Need to deal board cards for new street
        match state.street {
            Street::Flop if state.board.len() == 0 => true,
            Street::Turn if state.board.len() == 3 => true,
            Street::River if state.board.len() == 4 => true,
            _ => false,
        }
    }
}

impl Default for SBvsBBFullGame {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for SBvsBBFullGame {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SBvsBBFullGame")
            .field("stack_bb", &self.config.stack_bb)
            .finish()
    }
}

impl Game for SBvsBBFullGame {
    type State = PokerState;
    type Action = PokerAction;
    type InfoState = PokerInfoState;

    fn initial_state(&self) -> Self::State {
        PokerState::new_hu(
            [self.config.stack_bb, self.config.stack_bb],
            self.config.sb_amount,
            self.config.bb_amount,
        )
    }

    fn is_terminal(&self, state: &Self::State) -> bool {
        state.is_terminal
    }

    fn get_payoff(&self, state: &Self::State, player: usize) -> f64 {
        debug_assert!(self.is_terminal(state), "get_payoff called on non-terminal state");

        let pos = HUPosition::from_index(player);

        // Handle fold
        if let Some(folder) = state.folded {
            if folder == pos {
                // This player folded, lost their investment
                return -state.invested_total[player];
            } else {
                // Opponent folded, won the pot minus our investment
                return state.pot - state.invested_total[player];
            }
        }

        // Handle all-in runouts or river showdown
        if state.both_all_in() || state.street == Street::River || state.street == Street::Showdown {
            // We need to run out the remaining board if not complete
            // For CFR, we should already have dealt all cards in sample_chance
            // So just evaluate the showdown

            match self.determine_showdown_winner(state) {
                Some(winner) => {
                    if winner == pos {
                        state.pot - state.invested_total[player]
                    } else {
                        -state.invested_total[player]
                    }
                }
                None => {
                    // Tie - split pot
                    (state.pot / 2.0) - state.invested_total[player]
                }
            }
        } else {
            // Shouldn't reach here
            0.0
        }
    }

    fn current_player(&self, state: &Self::State) -> Option<usize> {
        if self.is_terminal(state) || self.is_chance(state) {
            None
        } else {
            state.to_act.map(|p| p.index())
        }
    }

    fn num_players(&self) -> usize {
        2
    }

    fn available_actions(&self, state: &Self::State) -> Vec<Self::Action> {
        if self.is_terminal(state) || self.is_chance(state) {
            return vec![];
        }
        self.betting.available_actions(state)
    }

    fn apply_action(&self, state: &Self::State, action: &Self::Action) -> Self::State {
        state.apply(*action)
    }

    fn info_state(&self, state: &Self::State) -> Self::InfoState {
        PokerInfoState::from_state(state, &self.abstraction)
            .expect("Failed to create info state")
    }

    fn is_chance(&self, state: &Self::State) -> bool {
        self.needs_deal(state)
    }

    fn sample_chance<R: Rng>(&self, state: &Self::State, rng: &mut R) -> Self::State {
        debug_assert!(self.is_chance(state), "sample_chance called on non-chance state");

        let mut new_state = state.clone();

        // Deal hole cards if needed
        if new_state.hands[0].is_none() || new_state.hands[1].is_none() {
            new_state.deck.shuffle(rng);

            let c1 = new_state.deck.deal().unwrap();
            let c2 = new_state.deck.deal().unwrap();
            let sb_hand = HoleCards::new(c1, c2);

            let c3 = new_state.deck.deal().unwrap();
            let c4 = new_state.deck.deal().unwrap();
            let bb_hand = HoleCards::new(c3, c4);

            new_state.hands = [Some(sb_hand), Some(bb_hand)];
            new_state.to_act = Some(HUPosition::SB);

            return new_state;
        }

        // Deal board cards based on street
        new_state.deck.shuffle(rng);

        match new_state.street {
            Street::Flop if new_state.board.len() == 0 => {
                new_state.deal_flop();
            }
            Street::Turn if new_state.board.len() == 3 => {
                new_state.deal_turn();
            }
            Street::River if new_state.board.len() == 4 => {
                new_state.deal_river();
            }
            _ => {}
        }

        // If both all-in and board complete, mark as terminal
        if new_state.both_all_in() && new_state.board.len() == 5 {
            new_state.is_terminal = true;
            new_state.to_act = None;
        }

        new_state
    }

    fn action_name(&self, action: &Self::Action) -> String {
        format!("{}", action)
    }

    fn state_description(&self, state: &Self::State) -> String {
        format!("{}", state)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cfr::{CFRConfig, CFRSolver};
    use crate::cfr::game::InfoState;

    #[test]
    fn test_initial_state() {
        let game = SBvsBBFullGame::new();
        let state = game.initial_state();

        assert!(!game.is_terminal(&state));
        assert!(game.is_chance(&state)); // Need to deal cards
        assert_eq!(state.street, Street::Preflop);
    }

    #[test]
    fn test_deal_hole_cards() {
        let game = SBvsBBFullGame::new();
        let state = game.initial_state();

        let mut rng = rand::thread_rng();
        let dealt = game.sample_chance(&state, &mut rng);

        assert!(!game.is_chance(&dealt));
        assert!(dealt.hands[0].is_some());
        assert!(dealt.hands[1].is_some());
        assert_eq!(game.current_player(&dealt), Some(0)); // SB acts first
    }

    #[test]
    fn test_fold_payoff() {
        let game = SBvsBBFullGame::new();
        let mut state = game.initial_state();

        let mut rng = rand::thread_rng();
        state = game.sample_chance(&state, &mut rng);

        // SB folds
        state = game.apply_action(&state, &PokerAction::Fold);

        assert!(game.is_terminal(&state));

        // SB loses 0.5bb (their posted blind)
        let sb_payoff = game.get_payoff(&state, 0);
        assert!((sb_payoff - (-0.5)).abs() < 0.01, "SB payoff {} should be -0.5", sb_payoff);

        // BB wins SB's blind (0.5bb profit)
        let bb_payoff = game.get_payoff(&state, 1);
        assert!((bb_payoff - 0.5).abs() < 0.01, "BB payoff {} should be 0.5", bb_payoff);
    }

    #[test]
    fn test_limp_check_to_flop() {
        let game = SBvsBBFullGame::new();
        let mut state = game.initial_state();

        let mut rng = rand::thread_rng();
        state = game.sample_chance(&state, &mut rng);

        // SB limps
        state = game.apply_action(&state, &PokerAction::Call);

        // BB checks
        state = game.apply_action(&state, &PokerAction::Check);

        assert_eq!(state.street, Street::Flop);
        assert!(game.is_chance(&state)); // Need to deal flop
    }

    #[test]
    fn test_full_hand_to_showdown() {
        let game = SBvsBBFullGame::fast();
        let mut state = game.initial_state();

        let mut rng = rand::thread_rng();

        // Deal hole cards
        state = game.sample_chance(&state, &mut rng);

        // Preflop: SB raises, BB calls
        let actions = game.available_actions(&state);
        let raise = actions.iter().find(|a| matches!(a, PokerAction::Raise(_))).unwrap();
        state = game.apply_action(&state, raise);
        state = game.apply_action(&state, &PokerAction::Call);

        // Flop
        assert!(game.is_chance(&state));
        state = game.sample_chance(&state, &mut rng);

        // Check-check on flop
        state = game.apply_action(&state, &PokerAction::Check);
        state = game.apply_action(&state, &PokerAction::Check);

        // Turn
        assert!(game.is_chance(&state));
        state = game.sample_chance(&state, &mut rng);

        // Check-check on turn
        state = game.apply_action(&state, &PokerAction::Check);
        state = game.apply_action(&state, &PokerAction::Check);

        // River
        assert!(game.is_chance(&state));
        state = game.sample_chance(&state, &mut rng);

        // Check-check on river
        state = game.apply_action(&state, &PokerAction::Check);
        state = game.apply_action(&state, &PokerAction::Check);

        assert!(game.is_terminal(&state));

        // Verify payoffs sum to zero
        let sb_payoff = game.get_payoff(&state, 0);
        let bb_payoff = game.get_payoff(&state, 1);
        assert!((sb_payoff + bb_payoff).abs() < 0.01,
            "Payoffs should sum to zero: {} + {} = {}", sb_payoff, bb_payoff, sb_payoff + bb_payoff);
    }

    #[test]
    fn test_all_in_preflop() {
        let game = SBvsBBFullGame::fast();
        let mut state = game.initial_state();

        let mut rng = rand::thread_rng();
        state = game.sample_chance(&state, &mut rng);

        // SB all-in
        state = game.apply_action(&state, &PokerAction::AllIn);

        // BB calls
        state = game.apply_action(&state, &PokerAction::Call);

        // Need to deal out the board
        while game.is_chance(&state) {
            state = game.sample_chance(&state, &mut rng);
        }

        assert!(game.is_terminal(&state));
        assert!(state.both_all_in());
    }

    #[test]
    fn test_info_state_generation() {
        let game = SBvsBBFullGame::fast();
        let mut state = game.initial_state();

        let mut rng = rand::thread_rng();
        state = game.sample_chance(&state, &mut rng);

        let info = game.info_state(&state);

        // Should be SB's turn on preflop
        assert_eq!(info.position(), HUPosition::SB);
        assert_eq!(info.street(), Street::Preflop);

        // Key should be well-formed
        let key = info.key();
        assert!(key.starts_with("P0S0B"), "Key should start with P0S0B, got {}", key);
    }

    #[test]
    fn test_cfr_solver_integration() {
        let game = SBvsBBFullGame::fast();
        let config = CFRConfig::default().with_seed(42);
        let mut solver = CFRSolver::new(game, config);

        // Run a few iterations to verify integration works
        solver.train(100);

        // Should have discovered some info sets
        assert!(solver.num_info_sets() > 0,
            "Should have discovered info sets, got {}", solver.num_info_sets());
    }

    #[test]
    fn test_available_actions_preflop() {
        let game = SBvsBBFullGame::new();
        let mut state = game.initial_state();

        let mut rng = rand::thread_rng();
        state = game.sample_chance(&state, &mut rng);

        let actions = game.available_actions(&state);

        // SB should be able to: Fold, Call (limp), Raise, possibly All-in
        assert!(actions.iter().any(|a| matches!(a, PokerAction::Fold)));
        assert!(actions.iter().any(|a| matches!(a, PokerAction::Call)));
        assert!(actions.iter().any(|a| matches!(a, PokerAction::Raise(_))));
    }

    #[test]
    fn test_available_actions_postflop() {
        let game = SBvsBBFullGame::new();
        let mut state = game.initial_state();

        let mut rng = rand::thread_rng();

        // Get to flop
        state = game.sample_chance(&state, &mut rng);
        state = game.apply_action(&state, &PokerAction::Call);
        state = game.apply_action(&state, &PokerAction::Check);
        state = game.sample_chance(&state, &mut rng);

        let actions = game.available_actions(&state);

        // SB on flop should be able to: Check, Bet
        assert!(actions.iter().any(|a| matches!(a, PokerAction::Check)));
        assert!(actions.iter().any(|a| matches!(a, PokerAction::Bet(_))));
        // Should NOT be able to fold when not facing bet
        assert!(!actions.iter().any(|a| matches!(a, PokerAction::Fold)));
    }
}
