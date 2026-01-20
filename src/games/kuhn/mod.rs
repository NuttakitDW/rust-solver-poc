//! Kuhn Poker implementation for CFR validation.
//!
//! Kuhn Poker is a simplified poker game used to validate CFR implementations
//! because it has a known, mathematically proven Nash equilibrium.
//!
//! ## Game Rules
//!
//! - 3 cards: Jack (0), Queen (1), King (2)
//! - 2 players, each antes 1 chip
//! - Each player receives 1 card
//! - Player 1 acts first: Pass or Bet (1 chip)
//! - Player 2 responds based on P1's action
//! - Higher card wins at showdown
//!
//! ## Game Tree
//!
//! ```text
//! P1 (first to act)
//! ├── Pass
//! │   └── P2
//! │       ├── Pass → Showdown (pot = 2)
//! │       └── Bet
//! │           └── P1
//! │               ├── Pass → P2 wins (pot = 3)
//! │               └── Bet → Showdown (pot = 4)
//! └── Bet
//!     └── P2
//!         ├── Pass → P1 wins (pot = 3)
//!         └── Bet → Showdown (pot = 4)
//! ```
//!
//! ## Known Nash Equilibrium
//!
//! - **Player 1 with Jack**: Bet with probability α ≈ 1/3
//! - **Player 1 with Queen**: Always Pass
//! - **Player 1 with King**: Bet with probability 3α ≈ 1
//! - **Player 2 facing Bet with Jack**: Always Fold
//! - **Player 2 facing Bet with Queen**: Call with probability 1/3
//! - **Player 2 facing Bet with King**: Always Call
//!
//! **Expected Value**: Player 1 EV = -1/18 ≈ -0.0556

use rand::Rng;
use std::fmt;

use crate::cfr::game::{Action, Game, GameState, InfoState};

/// Actions in Kuhn Poker.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum KuhnAction {
    /// Pass (check if no bet, fold if facing bet)
    Pass,
    /// Bet (or call if facing bet)
    Bet,
}

impl Action for KuhnAction {
    fn to_string(&self) -> String {
        match self {
            KuhnAction::Pass => "p".to_string(),
            KuhnAction::Bet => "b".to_string(),
        }
    }
}

impl fmt::Display for KuhnAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            KuhnAction::Pass => write!(f, "Pass"),
            KuhnAction::Bet => write!(f, "Bet"),
        }
    }
}

/// Information state in Kuhn Poker.
///
/// What a player knows: their card and the action history.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct KuhnInfoState {
    /// Player's card (0=Jack, 1=Queen, 2=King)
    pub card: u8,
    /// Action history as string (e.g., "pb" = pass then bet)
    pub history: String,
}

impl InfoState for KuhnInfoState {
    fn key(&self) -> String {
        format!("{}:{}", self.card, self.history)
    }
}

impl fmt::Display for KuhnInfoState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let card_name = match self.card {
            0 => "J",
            1 => "Q",
            2 => "K",
            _ => "?",
        };
        write!(f, "{}|{}", card_name, self.history)
    }
}

/// Complete game state in Kuhn Poker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KuhnState {
    /// Cards dealt to each player (0=Jack, 1=Queen, 2=King)
    /// cards[0] is Player 1's card, cards[1] is Player 2's card
    pub cards: [u8; 2],
    /// Action history as string
    pub history: String,
    /// Amount each player has invested in the pot
    pub pot: [i32; 2],
    /// Whether cards have been dealt (for chance node handling)
    pub dealt: bool,
}

impl GameState for KuhnState {}

impl Default for KuhnState {
    fn default() -> Self {
        Self {
            cards: [0, 0],
            history: String::new(),
            pot: [1, 1], // Both ante 1
            dealt: false,
        }
    }
}

impl fmt::Display for KuhnState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let cards: Vec<&str> = self.cards.iter().map(|&c| match c {
            0 => "J",
            1 => "Q",
            2 => "K",
            _ => "?",
        }).collect();
        write!(f, "P1:{} P2:{} History:{} Pot:{:?}", cards[0], cards[1], self.history, self.pot)
    }
}

/// Kuhn Poker game.
#[derive(Debug, Clone, Default)]
pub struct KuhnPoker;

impl KuhnPoker {
    /// Create a new Kuhn Poker game.
    pub fn new() -> Self {
        Self
    }

    /// Get card name for display.
    pub fn card_name(card: u8) -> &'static str {
        match card {
            0 => "Jack",
            1 => "Queen",
            2 => "King",
            _ => "Unknown",
        }
    }
}

impl Game for KuhnPoker {
    type State = KuhnState;
    type Action = KuhnAction;
    type InfoState = KuhnInfoState;

    fn initial_state(&self) -> Self::State {
        KuhnState::default()
    }

    fn is_terminal(&self, state: &Self::State) -> bool {
        let h = &state.history;
        // Terminal states:
        // "pp" - both pass, showdown
        // "pbp" - pass, bet, fold
        // "pbb" - pass, bet, call
        // "bp" - bet, fold
        // "bb" - bet, call
        matches!(h.as_str(), "pp" | "pbp" | "pbb" | "bp" | "bb")
    }

    fn get_payoff(&self, state: &Self::State, player: usize) -> f64 {
        debug_assert!(self.is_terminal(state), "get_payoff called on non-terminal state");

        let h = &state.history;
        let p0_card = state.cards[0];
        let p1_card = state.cards[1];

        // Calculate payoff for player 0 (player 1's payoff is negation)
        let p0_payoff: f64 = match h.as_str() {
            "pp" => {
                // Showdown after both pass - pot is 2 (1+1 ante)
                if p0_card > p1_card {
                    1.0 // Win opponent's ante
                } else {
                    -1.0 // Lose own ante
                }
            }
            "bp" => {
                // Player 1 bet, player 2 folded
                1.0 // P0 wins P1's ante
            }
            "pbp" => {
                // Player 1 passed, player 2 bet, player 1 folded
                -1.0 // P0 loses own ante
            }
            "bb" | "pbb" => {
                // Showdown after bet-call - pot is 4 (2+2)
                if p0_card > p1_card {
                    2.0 // Win opponent's 2 chips
                } else {
                    -2.0 // Lose own 2 chips
                }
            }
            _ => 0.0,
        };

        if player == 0 {
            p0_payoff
        } else {
            -p0_payoff
        }
    }

    fn current_player(&self, state: &Self::State) -> Option<usize> {
        if self.is_terminal(state) {
            return None;
        }

        if self.is_chance(state) {
            return None;
        }

        // Player alternates: P0 at even history length, P1 at odd
        // But after "pb", P0 acts again
        let h = &state.history;
        match h.as_str() {
            "" => Some(0),    // P0 acts first
            "p" => Some(1),   // P1 responds to pass
            "b" => Some(1),   // P1 responds to bet
            "pb" => Some(0),  // P0 responds to P1's bet after pass
            _ => None,        // Terminal
        }
    }

    fn num_players(&self) -> usize {
        2
    }

    fn available_actions(&self, state: &Self::State) -> Vec<Self::Action> {
        if self.is_terminal(state) || self.is_chance(state) {
            return vec![];
        }
        // Both actions always available when not terminal
        vec![KuhnAction::Pass, KuhnAction::Bet]
    }

    fn apply_action(&self, state: &Self::State, action: &Self::Action) -> Self::State {
        let mut new_state = state.clone();

        match action {
            KuhnAction::Pass => {
                new_state.history.push('p');
            }
            KuhnAction::Bet => {
                new_state.history.push('b');
                // Add 1 to current player's pot contribution
                let player = self.current_player(state).unwrap();
                new_state.pot[player] += 1;
            }
        }

        new_state
    }

    fn info_state(&self, state: &Self::State) -> Self::InfoState {
        let player = self.current_player(state).unwrap_or(0);
        KuhnInfoState {
            card: state.cards[player],
            history: state.history.clone(),
        }
    }

    fn is_chance(&self, state: &Self::State) -> bool {
        // Chance node is when cards haven't been dealt yet
        !state.dealt
    }

    fn sample_chance<R: Rng>(&self, state: &Self::State, rng: &mut R) -> Self::State {
        debug_assert!(self.is_chance(state), "sample_chance called on non-chance state");

        // Deal cards: shuffle [0,1,2] and deal first two
        let mut cards = [0u8, 1, 2];

        // Fisher-Yates shuffle
        for i in (1..3).rev() {
            let j = rng.gen_range(0..=i);
            cards.swap(i, j);
        }

        KuhnState {
            cards: [cards[0], cards[1]],
            history: String::new(),
            pot: [1, 1],
            dealt: true,
        }
    }

    fn action_name(&self, action: &Self::Action) -> String {
        match action {
            KuhnAction::Pass => "Pass".to_string(),
            KuhnAction::Bet => "Bet".to_string(),
        }
    }

    fn state_description(&self, state: &Self::State) -> String {
        format!("{}", state)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cfr::{CFRConfig, CFRSolver};

    #[test]
    fn test_kuhn_game_tree() {
        let game = KuhnPoker::new();

        // Test initial state
        let state = game.initial_state();
        assert!(!state.dealt);
        assert!(game.is_chance(&state));

        // Test after dealing (manually set dealt state)
        let dealt_state = KuhnState {
            cards: [2, 0], // K vs J
            history: String::new(),
            pot: [1, 1],
            dealt: true,
        };
        assert!(!game.is_chance(&dealt_state));
        assert!(!game.is_terminal(&dealt_state));
        assert_eq!(game.current_player(&dealt_state), Some(0));

        // Test actions
        let actions = game.available_actions(&dealt_state);
        assert_eq!(actions.len(), 2);
        assert!(actions.contains(&KuhnAction::Pass));
        assert!(actions.contains(&KuhnAction::Bet));
    }

    #[test]
    fn test_kuhn_terminal_payoffs() {
        let game = KuhnPoker::new();

        // Test "pp" - both pass, higher card wins
        let pp_state = KuhnState {
            cards: [2, 0], // K vs J
            history: "pp".to_string(),
            pot: [1, 1],
            dealt: true,
        };
        assert!(game.is_terminal(&pp_state));
        assert_eq!(game.get_payoff(&pp_state, 0), 1.0); // K wins
        assert_eq!(game.get_payoff(&pp_state, 1), -1.0);

        // Test "bp" - bet then fold
        let bp_state = KuhnState {
            cards: [0, 2], // J vs K
            history: "bp".to_string(),
            pot: [2, 1],
            dealt: true,
        };
        assert!(game.is_terminal(&bp_state));
        assert_eq!(game.get_payoff(&bp_state, 0), 1.0); // J wins by fold

        // Test "bb" - bet then call, showdown
        let bb_state = KuhnState {
            cards: [0, 2], // J vs K
            history: "bb".to_string(),
            pot: [2, 2],
            dealt: true,
        };
        assert!(game.is_terminal(&bb_state));
        assert_eq!(game.get_payoff(&bb_state, 0), -2.0); // J loses showdown
        assert_eq!(game.get_payoff(&bb_state, 1), 2.0);  // K wins showdown
    }

    #[test]
    fn test_kuhn_info_states() {
        let game = KuhnPoker::new();

        let state = KuhnState {
            cards: [1, 2], // Q vs K
            history: "p".to_string(),
            pot: [1, 1],
            dealt: true,
        };

        // Current player is P1 (index 1)
        assert_eq!(game.current_player(&state), Some(1));

        // Info state should show P1's card (K=2) and history
        let info = game.info_state(&state);
        assert_eq!(info.card, 2);
        assert_eq!(info.history, "p");
        assert_eq!(info.key(), "2:p");
    }

    #[test]
    fn test_kuhn_cfr_convergence() {
        let game = KuhnPoker::new();
        let config = CFRConfig::default().with_seed(42);
        let mut solver = CFRSolver::new(game, config);

        // Run 50,000 iterations for better convergence
        // MCCFR with external sampling needs more iterations than full CFR
        solver.train(50_000);

        // Check that we discovered the expected info sets
        // There are 12 info sets: 3 cards × 4 possible histories
        // (empty, "p", "b", "pb" for decision points)
        assert!(solver.num_info_sets() > 0);

        // Expected Nash Equilibrium for Kuhn Poker:
        // P1 Jack:  Pass=0.667, Bet=0.333 (bluff with probability α = 1/3)
        // P1 Queen: Pass=1.000, Bet=0.000 (never bet)
        // P1 King:  Pass=0.000, Bet=1.000 (always bet)

        // Check P1's strategy with Jack at root (should bet ~1/3)
        let jack_strategy = solver.get_average_strategy("0:", 2);
        println!("Jack strategy: Pass={:.3}, Bet={:.3}", jack_strategy[0], jack_strategy[1]);

        // Jack should bet with probability around 1/3 (±0.15 for convergence tolerance)
        // Index 0 = Pass, Index 1 = Bet
        let jack_bet_prob = jack_strategy[1];
        assert!(
            jack_bet_prob > 0.15 && jack_bet_prob < 0.5,
            "Jack bet probability {} should be near 1/3",
            jack_bet_prob
        );

        // Check P1's strategy with Queen at root (should always pass)
        let queen_strategy = solver.get_average_strategy("1:", 2);
        println!("Queen strategy: Pass={:.3}, Bet={:.3}", queen_strategy[0], queen_strategy[1]);

        // Queen should mostly pass (>95%)
        assert!(
            queen_strategy[0] > 0.95,
            "Queen pass probability {} should be near 1.0",
            queen_strategy[0]
        );

        // Check P1's strategy with King at root (should mostly bet)
        let king_strategy = solver.get_average_strategy("2:", 2);
        println!("King strategy: Pass={:.3}, Bet={:.3}", king_strategy[0], king_strategy[1]);

        // King should bet more than 50% and more than Jack
        assert!(
            king_strategy[1] > 0.5,
            "King bet probability {} should be >50%",
            king_strategy[1]
        );
        assert!(
            king_strategy[1] > jack_bet_prob,
            "King should bet more often than Jack"
        );

        // Check P2's strategies facing a bet
        let p2_jack_vs_bet = solver.get_average_strategy("0:b", 2);
        let p2_queen_vs_bet = solver.get_average_strategy("1:b", 2);
        let p2_king_vs_bet = solver.get_average_strategy("2:b", 2);

        println!("P2 Jack facing bet: Fold={:.3}, Call={:.3}", p2_jack_vs_bet[0], p2_jack_vs_bet[1]);
        println!("P2 Queen facing bet: Fold={:.3}, Call={:.3}", p2_queen_vs_bet[0], p2_queen_vs_bet[1]);
        println!("P2 King facing bet: Fold={:.3}, Call={:.3}", p2_king_vs_bet[0], p2_king_vs_bet[1]);

        // P2 with Jack should always fold (>95%)
        assert!(
            p2_jack_vs_bet[0] > 0.95,
            "P2 Jack should fold to bet"
        );

        // P2 with King should always call (>95%)
        assert!(
            p2_king_vs_bet[1] > 0.95,
            "P2 King should call bet"
        );

        // P2 with Queen should call about 1/3 of the time (±0.15)
        assert!(
            p2_queen_vs_bet[1] > 0.2 && p2_queen_vs_bet[1] < 0.5,
            "P2 Queen call probability {} should be near 1/3",
            p2_queen_vs_bet[1]
        );

        println!("Kuhn Poker CFR convergence test passed!");
    }
}
