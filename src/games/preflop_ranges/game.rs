//! Preflop range game implementation.
//!
//! Simplified preflop game for solving opening and defense ranges.

use std::collections::HashMap;
use rand::Rng;

use crate::cfr::game::{Game, GameState, Action, InfoState};
use super::state::{PreflopRangeState, Position, Scenario, ActionType};
use super::{HAND_NAMES, hand_class_to_grid, grid_to_hand_name};

/// Configuration for preflop range solving
#[derive(Debug, Clone)]
pub struct PreflopRangeConfig {
    pub stack_bb: f64,
    pub sb: f64,
    pub bb: f64,
    pub ante: f64,
    pub open_size: f64,      // e.g., 2.3bb
    pub threebet_size: f64,  // e.g., 3x open
    pub fourbet_size: f64,   // e.g., 2.5x 3bet
}

impl Default for PreflopRangeConfig {
    fn default() -> Self {
        Self {
            stack_bb: 50.0,
            sb: 0.5,
            bb: 1.0,
            ante: 0.12,
            open_size: 2.3,
            threebet_size: 3.0,
            fourbet_size: 2.5,
        }
    }
}

/// Preflop range action
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RangeAction(pub ActionType);

impl Action for RangeAction {
    fn to_string(&self) -> String {
        self.0.name().to_string()
    }
}

/// Info state for preflop ranges - just scenario + hand
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RangeInfoState {
    scenario_name: String,
    hand_class: u8,
}

impl InfoState for RangeInfoState {
    fn key(&self) -> String {
        format!("{}|{}", self.scenario_name, self.hand_class)
    }
}

impl GameState for PreflopRangeState {}

/// Preflop range game for a specific scenario
pub struct PreflopRangeGame {
    pub config: PreflopRangeConfig,
    pub scenario: Scenario,
    /// Equity lookup table: hand_class -> equity vs villain range
    equity_table: [f64; 169],
}

impl PreflopRangeGame {
    pub fn new(scenario: Scenario, config: PreflopRangeConfig) -> Self {
        let equity_table = compute_equity_table(&scenario);
        Self { config, scenario, equity_table }
    }

    /// Get available actions for this scenario
    fn get_actions(&self) -> Vec<RangeAction> {
        match &self.scenario {
            Scenario::RFI { .. } => vec![
                RangeAction(ActionType::Fold),
                RangeAction(ActionType::Raise),
            ],
            Scenario::VsRFI { .. } => vec![
                RangeAction(ActionType::Fold),
                RangeAction(ActionType::Call),
                RangeAction(ActionType::Raise), // 3bet
            ],
            Scenario::Vs3Bet { .. } => vec![
                RangeAction(ActionType::Fold),
                RangeAction(ActionType::Call),
                RangeAction(ActionType::Raise), // 4bet
            ],
            Scenario::Vs4Bet { .. } => vec![
                RangeAction(ActionType::Fold),
                RangeAction(ActionType::Call),
                RangeAction(ActionType::AllIn),
            ],
            Scenario::Vs5Bet { .. } => vec![
                RangeAction(ActionType::Fold),
                RangeAction(ActionType::Call),
            ],
            _ => vec![
                RangeAction(ActionType::Fold),
                RangeAction(ActionType::Raise),
            ],
        }
    }

    /// Calculate EV for an action
    /// Uses position-based equity vs villain's calling range (not vs random)
    fn calculate_ev(&self, state: &PreflopRangeState, action: ActionType) -> f64 {
        let raw_equity = self.equity_table[state.hand_class as usize];
        let pot = self.config.sb + self.config.bb + self.config.ante * 8.0;
        let open_size = self.config.open_size;

        match (&self.scenario, action) {
            (Scenario::RFI { position }, ActionType::Fold) => {
                // Folding loses any posted blinds/ante
                match position {
                    Position::SB => -self.config.sb,
                    Position::BB => -self.config.bb,
                    _ => 0.0,
                }
            }
            (Scenario::RFI { position }, ActionType::Raise) => {
                // Position-based parameters calibrated to match HRC ranges
                // HRC typical RFI: UTG 13%, EP 15%, MP 18%, HJ 22%, CO 28%, BU 45%, SB 35%
                let (fold_equity, three_bet_freq, eq_realization, equity_penalty, min_equity) = match position {
                    Position::UTG => (0.75, 0.12, 0.65, 0.22, 0.68), // ~13% range - very tight
                    Position::EP  => (0.72, 0.10, 0.68, 0.20, 0.66), // ~15% range
                    Position::MP  => (0.68, 0.09, 0.72, 0.18, 0.62), // ~18% range
                    Position::HJ  => (0.62, 0.08, 0.78, 0.16, 0.56), // ~22% range
                    Position::CO  => (0.55, 0.07, 0.85, 0.14, 0.50), // ~28% range
                    Position::BU  => (0.45, 0.10, 0.92, 0.10, 0.42), // ~45% range
                    Position::SB  => (0.50, 0.15, 0.70, 0.16, 0.48), // ~35% range
                    Position::BB  => (0.0, 0.0, 0.75, 0.12, 0.70),   // N/A
                };

                // Hand must have minimum equity to even consider
                if raw_equity < min_equity {
                    return -open_size * 2.0; // Very negative EV for weak hands
                }

                // When called, villain has a tighter range - reduce our equity significantly
                let called_equity = (raw_equity - equity_penalty).max(0.25);

                // EV when called (postflop play, OOP penalty)
                let called_ev = eq_realization * called_equity * (pot + open_size * 2.0) - open_size;

                // EV when facing 3bet (usually fold or lose more)
                let face_3bet_ev = -open_size * 0.90;

                let play_ev = (1.0 - three_bet_freq) * called_ev + three_bet_freq * face_3bet_ev;
                fold_equity * pot + (1.0 - fold_equity) * play_ev
            }
            (Scenario::VsRFI { hero, villain }, ActionType::Fold) => {
                // Folding loses posted blind
                match hero {
                    Position::BB => -self.config.bb,
                    Position::SB => -self.config.sb,
                    _ => 0.0,
                }
            }
            (Scenario::VsRFI { hero, villain }, ActionType::Call) => {
                // Get villain's opening range width to adjust our equity
                let villain_range_width = match villain {
                    Position::UTG => 0.12,
                    Position::EP => 0.15,
                    Position::MP => 0.20,
                    Position::HJ => 0.25,
                    Position::CO => 0.30,
                    Position::BU => 0.45,
                    Position::SB => 0.50,
                    _ => 0.30,
                };

                // Wider villain = we can defend wider
                // min_equity to call: UTG open needs ~0.50, BU open needs ~0.38
                let min_call_equity = 0.55 - villain_range_width * 0.4;

                if raw_equity < min_call_equity {
                    return -self.config.bb * 2.0;
                }

                // Tighter villain = stronger range = we need more equity
                let equity_reduction = 0.22 - villain_range_width * 0.20;
                let effective_equity = (raw_equity - equity_reduction).max(0.28);

                // OOP penalty - significant
                let eq_realization = match hero {
                    Position::BB => 0.65,
                    Position::SB => 0.60,
                    _ => 0.75,
                };

                let call_size = open_size - match hero {
                    Position::BB => self.config.bb,
                    Position::SB => self.config.sb,
                    _ => 0.0,
                };

                eq_realization * effective_equity * (pot + open_size * 2.0) - call_size
            }
            (Scenario::VsRFI { hero, villain }, ActionType::Raise) => {
                // 3bet - needs strong hands
                // ~8-12% 3bet range typically
                let threbet_size = open_size * self.config.threebet_size;

                // Wider villain = we can 3bet wider
                let villain_range_width = match villain {
                    Position::UTG => 0.12,
                    Position::EP => 0.15,
                    Position::MP => 0.20,
                    Position::HJ => 0.25,
                    Position::CO => 0.30,
                    Position::BU => 0.45,
                    Position::SB => 0.50,
                    _ => 0.30,
                };

                // Min equity to 3bet: ~0.55 vs UTG, ~0.48 vs BU
                let min_3bet_equity = 0.58 - villain_range_width * 0.25;
                if raw_equity < min_3bet_equity {
                    return -threbet_size;
                }

                let fold_equity = 0.55;
                let eq_realization = match hero {
                    Position::BB => 0.80,
                    Position::SB => 0.75,
                    _ => 0.85,
                };

                // When called, villain has premium hands - big equity reduction
                let called_equity = (raw_equity - 0.22).max(0.32);

                let win_pot = pot + open_size;
                let called_ev = eq_realization * called_equity * (pot + threbet_size * 2.0) - threbet_size;

                fold_equity * win_pot + (1.0 - fold_equity) * called_ev
            }
            (Scenario::Vs3Bet { .. }, ActionType::Fold) => {
                -open_size
            }
            (Scenario::Vs3Bet { .. }, ActionType::Call) => {
                let threbet_size = open_size * self.config.threebet_size;
                let call_size = threbet_size - open_size;

                // 3better has strong range
                let effective_equity = (raw_equity - 0.15).max(0.30);

                // Need ~55% raw equity to call 3bet
                if raw_equity < 0.55 {
                    return -call_size - open_size;
                }

                0.80 * effective_equity * (pot + threbet_size * 2.0) - call_size - open_size
            }
            (Scenario::Vs3Bet { .. }, ActionType::Raise) => {
                let threbet_size = open_size * self.config.threebet_size;
                let fourbet_size = threbet_size * self.config.fourbet_size;

                // Only premium hands (AA, KK, QQ, AK)
                if raw_equity < 0.65 {
                    return -fourbet_size - open_size;
                }

                let fold_equity = 0.50;
                let win_pot = pot + open_size + threbet_size;
                let called_ev = (raw_equity - 0.10) * (pot + fourbet_size * 2.0) - fourbet_size - open_size;

                fold_equity * win_pot + (1.0 - fold_equity) * called_ev
            }
            (_, ActionType::Fold) => 0.0,
            (_, ActionType::Call) => {
                let pot_after = pot * 2.0;
                0.70 * raw_equity * pot_after - pot / 4.0
            }
            (_, ActionType::Raise) | (_, ActionType::AllIn) => {
                let fold_equity = 0.40;
                let called_ev = (raw_equity - 0.15) * pot * 3.0 - pot;
                fold_equity * pot + (1.0 - fold_equity) * called_ev
            }
        }
    }
}

impl Clone for PreflopRangeGame {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            scenario: self.scenario.clone(),
            equity_table: self.equity_table,
        }
    }
}

impl Game for PreflopRangeGame {
    type State = PreflopRangeState;
    type Action = RangeAction;
    type InfoState = RangeInfoState;

    fn initial_state(&self) -> Self::State {
        // Hand class will be set by chance node
        PreflopRangeState::new(self.scenario.clone(), 0)
    }

    fn is_terminal(&self, state: &Self::State) -> bool {
        state.decided
    }

    fn get_payoff(&self, state: &Self::State, player: usize) -> f64 {
        if player != 0 {
            return -self.get_payoff(state, 0);
        }

        match state.action {
            Some(action) => self.calculate_ev(state, action),
            None => 0.0,
        }
    }

    fn current_player(&self, state: &Self::State) -> Option<usize> {
        if state.decided || state.hand_class == 0 && !state.decided {
            None // Terminal or chance
        } else {
            Some(0)
        }
    }

    fn num_players(&self) -> usize {
        2
    }

    fn available_actions(&self, state: &Self::State) -> Vec<Self::Action> {
        if state.decided {
            vec![]
        } else {
            self.get_actions()
        }
    }

    fn apply_action(&self, state: &Self::State, action: &Self::Action) -> Self::State {
        state.clone().with_action(action.0)
    }

    fn info_state(&self, state: &Self::State) -> Self::InfoState {
        RangeInfoState {
            scenario_name: self.scenario.name(),
            hand_class: state.hand_class,
        }
    }

    fn is_chance(&self, state: &Self::State) -> bool {
        !state.decided && state.hand_class == 0
    }

    fn sample_chance<R: Rng>(&self, state: &Self::State, rng: &mut R) -> Self::State {
        // Sample hand class weighted by combos
        let hand_class = sample_hand_class_weighted(rng);
        PreflopRangeState::new(self.scenario.clone(), hand_class)
    }

    fn action_name(&self, action: &Self::Action) -> String {
        action.0.name().to_string()
    }

    fn state_description(&self, state: &Self::State) -> String {
        let (row, col) = hand_class_to_grid(state.hand_class);
        let hand = grid_to_hand_name(row, col);
        format!("{}: {}", self.scenario.name(), hand)
    }
}

/// Compute playability table - scores calibrated to match HRC-style ranges
fn compute_equity_table(_scenario: &Scenario) -> [f64; 169] {
    let mut table = [0.0; 169];

    for class_idx in 0..169u8 {
        table[class_idx as usize] = compute_playability(class_idx);
    }

    table
}

/// Compute playability score for a hand class
/// Higher = more profitable to open. Based on HRC ranges.
fn compute_playability(class_idx: u8) -> f64 {
    let (rank1, rank2, suited) = decode_hand_class_for_playability(class_idx);

    // rank1 >= rank2, where A=12, K=11, Q=10, J=9, T=8, 9=7, ..., 2=0

    // Pairs - calibrated to match HRC
    // HRC UTG: AA-77=100%, 66=47%, 55=59%, 44-22=0%
    if rank1 == rank2 {
        return match rank1 {
            12 => 0.95, // AA
            11 => 0.92, // KK
            10 => 0.88, // QQ
            9  => 0.85, // JJ
            8  => 0.82, // TT
            7  => 0.78, // 99
            6  => 0.74, // 88 - should be in UTG range
            5  => 0.70, // 77 - should be in UTG range
            4  => 0.52, // 66 - marginal (HRC: 47%)
            3  => 0.50, // 55 - marginal (HRC: 59%)
            2  => 0.42, // 44 - fold UTG
            1  => 0.40, // 33 - fold UTG
            0  => 0.38, // 22 - fold UTG
            _ => 0.40,
        };
    }

    // Base score from high card
    let high_card_value = match rank1 {
        12 => 0.20, // A high
        11 => 0.15, // K high
        10 => 0.12, // Q high
        9  => 0.10, // J high
        8  => 0.08, // T high
        _  => 0.04, // 9 or lower
    };

    // Connectivity bonus (consecutive ranks)
    let gap = rank1 - rank2;
    let connectivity = match gap {
        1 => 0.15, // Connector (AK, KQ, etc.)
        2 => 0.08, // One-gapper
        3 => 0.04, // Two-gapper
        _ => 0.00,
    };

    // Broadway bonus (both cards T+)
    let broadway_bonus = if rank1 >= 8 && rank2 >= 8 { 0.20 } else { 0.0 };

    // Suited bonus
    let suited_bonus = if suited { 0.12 } else { 0.0 };

    // Ace bonus for suited aces (nut flush potential)
    // HRC values A7s=75%, A5s=49%, wheel aces have some value
    let ace_suited_bonus = if rank1 == 12 && suited {
        if rank2 >= 8 { 0.12 }      // ATs+ - premium
        else if rank2 >= 5 { 0.08 } // A9s-A6s - decent
        else { 0.06 }               // A5s-A2s - wheel potential
    } else { 0.0 };

    // Low card penalty
    let low_card_penalty = if rank2 <= 5 { 0.08 } else { 0.0 };

    // Calculate final score
    let base: f64 = high_card_value + connectivity + broadway_bonus + suited_bonus + ace_suited_bonus - low_card_penalty;

    // Scale to reasonable range
    (base + 0.30).min(0.85).max(0.25)
}

/// Decode hand class to (rank1, rank2, suited)
fn decode_hand_class_for_playability(class_idx: u8) -> (u8, u8, bool) {
    if class_idx < 13 {
        // Pairs: index 0-12 maps to 22-AA
        (class_idx, class_idx, false)
    } else if class_idx < 91 {
        // Suited: index 13-90 (78 hands)
        let idx = class_idx - 13;
        let (r1, r2) = decode_triangular_ranks(idx);
        (r1, r2, true)
    } else {
        // Offsuit: index 91-168 (78 hands)
        let idx = class_idx - 91;
        let (r1, r2) = decode_triangular_ranks(idx);
        (r1, r2, false)
    }
}

/// Decode triangular index to ranks (r1 > r2)
fn decode_triangular_ranks(idx: u8) -> (u8, u8) {
    // Triangular order: 32, 42, 43, 52, 53, 54, 62, ...
    // Row n (0-indexed) has n+1 entries, starting at rank n+1
    let mut r1 = 1u8; // Starting with rank 1 (3-high hands)
    let mut remaining = idx;
    while remaining >= r1 {
        remaining -= r1;
        r1 += 1;
    }
    // r1 is now the high card rank (1=3, 2=4, ..., 12=A)
    // remaining is the low card rank
    (r1, remaining)
}

/// Sample hand class weighted by number of combos
fn sample_hand_class_weighted<R: Rng>(rng: &mut R) -> u8 {
    // Combos: pairs=6, suited=4, offsuit=12
    let roll: u32 = rng.gen_range(0..1326);

    // Pairs: 13 * 6 = 78 combos (indices 0-12)
    if roll < 78 {
        return (roll / 6) as u8;
    }

    // Suited: 78 * 4 = 312 combos (indices 13-90)
    let roll = roll - 78;
    if roll < 312 {
        return 13 + (roll / 4) as u8;
    }

    // Offsuit: 78 * 12 = 936 combos (indices 91-168)
    let roll = roll - 312;
    91 + (roll / 12) as u8
}

/// Solve a scenario and return strategies for all 169 hands
pub fn solve_scenario(
    scenario: Scenario,
    config: &PreflopRangeConfig,
    iterations: u64,
) -> HashMap<u8, Vec<f64>> {
    use crate::cfr::{CFRConfig, CFRSolver};

    let game = PreflopRangeGame::new(scenario, config.clone());
    let cfr_config = CFRConfig::default()
        .with_cfr_plus(true)
        .with_linear_cfr(true);

    let mut solver = CFRSolver::new(game.clone(), cfr_config);
    solver.train(iterations);

    // Extract strategies for each hand class
    let mut strategies = HashMap::new();
    let actions = game.get_actions();
    let num_actions = actions.len();

    for hand_class in 0..169u8 {
        let key = format!("{}|{}", game.scenario.name(), hand_class);
        let strategy = solver.get_average_strategy(&key, num_actions);
        strategies.insert(hand_class, strategy);
    }

    strategies
}
