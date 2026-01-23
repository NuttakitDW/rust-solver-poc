//! 8-max preflop game implementation.
//!
//! This implements the Game trait for a preflop-only poker game.
//! Postflop value is estimated using equity realization.

use rand::Rng;

use crate::cfr::game::{Game, InfoState as InfoStateTrait};
use super::state::{PreflopState, Position8Max, BetLevel};
use super::action::{PreflopAction, bb_to_centi, centi_to_bb};
use super::equity::EquityCalculator;
use crate::games::preflop::config::PreflopConfig;

/// Configuration for the 8-max preflop game.
#[derive(Debug, Clone)]
pub struct Preflop8MaxConfig {
    /// Stack size in BB.
    pub stack_bb: f64,
    /// Small blind amount.
    pub sb_amount: f64,
    /// Big blind amount.
    pub bb_amount: f64,
    /// Ante per player.
    pub ante: f64,

    /// Open raise sizing (base + per_caller).
    pub open_size: (f64, f64),
    /// Open raise sizing from SB.
    pub open_size_sb: (f64, f64),
    /// 3-bet sizing (as multiplier of open + per_caller).
    pub threebet_size_ip: (f64, f64),
    pub threebet_size_oop: (f64, f64),
    /// 4-bet sizing (as % of pot).
    pub fourbet_pot_pct: f64,
    /// 5-bet sizing (as % of pot).
    pub fivebet_pot_pct: f64,

    /// All-in threshold (% of stack).
    pub allin_threshold_pct: f64,
    /// SPR below which all-in is always added.
    pub allin_spr_threshold: f64,

    /// Number of flats allowed at each level [RFI, facing_raise, 3bet, 4bet, 5bet].
    pub allowed_flats: [u8; 5],
    /// Allow cold calls (calling without previous involvement).
    pub allow_cold_calls: bool,
}

impl Default for Preflop8MaxConfig {
    fn default() -> Self {
        Self {
            stack_bb: 50.0,
            sb_amount: 0.5,
            bb_amount: 1.0,
            ante: 0.12,
            open_size: (2.3, 1.0),
            open_size_sb: (3.5, 1.0),
            threebet_size_ip: (2.5, 1.0),
            threebet_size_oop: (3.3, 1.0),
            fourbet_pot_pct: 0.90,
            fivebet_pot_pct: 1.20,
            allin_threshold_pct: 0.40,
            allin_spr_threshold: 7.0,
            allowed_flats: [0, 1, 1, 1, 0],
            allow_cold_calls: false,
        }
    }
}

impl Preflop8MaxConfig {
    /// Create config from JSON PreflopConfig.
    pub fn from_preflop_config(config: &PreflopConfig) -> Self {
        let stack = config.hand_data.stacks.values().next().copied().unwrap_or(50.0);

        Self {
            stack_bb: stack,
            sb_amount: config.blinds.sb,
            bb_amount: config.blinds.bb,
            ante: config.blinds.ante,
            open_size: (config.sizing.open.others.base, config.sizing.open.others.per_caller),
            open_size_sb: (config.sizing.open.sb.base, config.sizing.open.sb.per_caller),
            threebet_size_ip: (config.sizing.threebet.ip.base, config.sizing.threebet.ip.per_caller),
            threebet_size_oop: (config.sizing.threebet.bb_vs_other.base, config.sizing.threebet.bb_vs_other.per_caller),
            fourbet_pot_pct: config.sizing.fourbet.ip.percent_pot,
            fivebet_pot_pct: config.sizing.fivebet.ip.percent_pot,
            allin_threshold_pct: config.action_restrictions.preflop_allin_threshold / 100.0,
            allin_spr_threshold: config.action_restrictions.preflop_add_allin_spr,
            allowed_flats: config.action_restrictions.allowed_flats_per_raise,
            allow_cold_calls: config.action_restrictions.allow_cold_calls,
        }
    }
}

/// Information state for 8-max preflop.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PreflopInfoState {
    /// Position of the player.
    position: Position8Max,
    /// Hand class (0-168).
    hand_class: u8,
    /// Action history string.
    history: String,
}

impl InfoStateTrait for PreflopInfoState {
    fn key(&self) -> String {
        format!("P{}H{}:{}", self.position.index(), self.hand_class, self.history)
    }
}

/// 8-max preflop poker game.
#[derive(Clone)]
pub struct Preflop8MaxGame {
    config: Preflop8MaxConfig,
    equity_calc: EquityCalculator,
}

impl Preflop8MaxGame {
    /// Create a new game with default configuration.
    pub fn new() -> Self {
        Self {
            config: Preflop8MaxConfig::default(),
            equity_calc: EquityCalculator::default(),
        }
    }

    /// Create a game with custom configuration.
    pub fn with_config(config: Preflop8MaxConfig) -> Self {
        Self {
            config,
            equity_calc: EquityCalculator::default(),
        }
    }

    /// Create from JSON config.
    pub fn from_json_config(config: &PreflopConfig) -> Self {
        Self::with_config(Preflop8MaxConfig::from_preflop_config(config))
    }

    /// Get available actions for the current state.
    fn get_available_actions(&self, state: &PreflopState) -> Vec<PreflopAction> {
        let mut actions = Vec::new();

        let pos = match state.to_act {
            Some(p) => p,
            None => return actions,
        };

        let idx = pos.index();
        let stack = state.stacks[idx];
        let invested = state.invested[idx];
        let to_call = state.to_call - invested + state.invested[state.last_aggressor.map(|p| p.index()).unwrap_or(7)];
        let to_call = (to_call).max(0.0);

        // Always can fold if facing a bet
        if to_call > 0.001 {
            actions.push(PreflopAction::Fold);
        }

        // Can call if there's something to call (or check/limp)
        if to_call <= stack {
            actions.push(PreflopAction::Call);
        }

        // Calculate raise sizes based on bet level
        let raise_sizes = self.calculate_raise_sizes(state, pos);

        for size in raise_sizes {
            if size >= stack {
                // All-in instead
                if !actions.iter().any(|a| matches!(a, PreflopAction::AllIn)) {
                    actions.push(PreflopAction::AllIn);
                }
            } else {
                actions.push(PreflopAction::Raise(bb_to_centi(size)));
            }
        }

        // Add all-in if SPR is low or approaching threshold
        let spr = stack / state.pot;
        let remaining_pct = stack / self.config.stack_bb;

        if (spr <= self.config.allin_spr_threshold || remaining_pct <= self.config.allin_threshold_pct)
            && !actions.iter().any(|a| matches!(a, PreflopAction::AllIn))
        {
            actions.push(PreflopAction::AllIn);
        }

        actions
    }

    /// Calculate raise sizes based on bet level and position.
    fn calculate_raise_sizes(&self, state: &PreflopState, pos: Position8Max) -> Vec<f64> {
        let mut sizes = Vec::new();
        let stack = state.stacks[pos.index()];

        match state.bet_level {
            BetLevel::Unopened => {
                // Open raise
                let (base, per_caller) = if pos == Position8Max::SB {
                    self.config.open_size_sb
                } else {
                    self.config.open_size
                };
                let size = base + per_caller * state.num_callers as f64;
                sizes.push(size);
            }
            BetLevel::FacingRaise => {
                // 3-bet
                let is_ip = if let Some(aggressor) = state.last_aggressor {
                    pos.is_ip_vs(&aggressor)
                } else {
                    false
                };
                let (mult, per_caller) = if is_ip {
                    self.config.threebet_size_ip
                } else {
                    self.config.threebet_size_oop
                };
                let open_size = state.to_call;
                let size = open_size * mult + per_caller * state.num_callers as f64;
                sizes.push(size);
            }
            BetLevel::Facing3Bet => {
                // 4-bet (pot-based)
                let pot_after_call = state.pot + state.to_call;
                let size = pot_after_call * self.config.fourbet_pot_pct;
                sizes.push(size.max(state.to_call * 2.2));
            }
            BetLevel::Facing4Bet => {
                // 5-bet (pot-based)
                let pot_after_call = state.pot + state.to_call;
                let size = pot_after_call * self.config.fivebet_pot_pct;
                sizes.push(size.max(state.to_call * 2.2));
            }
            BetLevel::Facing5Bet | BetLevel::AllIn => {
                // Only all-in available
            }
        }

        // Filter sizes that exceed stack
        sizes.retain(|&s| s <= stack * 0.95); // Leave room for meaningful non-allin raise

        sizes
    }

    /// Apply an action to the state.
    fn apply_action(&self, state: &PreflopState, action: &PreflopAction) -> PreflopState {
        let mut new_state = state.clone();

        let pos = match state.to_act {
            Some(p) => p,
            None => return new_state,
        };
        let idx = pos.index();

        // Record action
        if !new_state.action_history.is_empty() {
            new_state.action_history.push('-');
        }
        new_state.action_history.push_str(&action.short_code());
        new_state.has_acted[idx] = true;

        match action {
            PreflopAction::Fold => {
                new_state.folded[idx] = true;

                // Check if only one player remains
                if new_state.active_players() == 1 {
                    new_state.is_terminal = true;
                    new_state.to_act = None;
                } else {
                    // Find next player to act
                    new_state.to_act = new_state.next_to_act(pos);
                    if new_state.to_act.is_none() || new_state.is_action_complete() {
                        new_state.is_terminal = true;
                        new_state.to_act = None;
                    }
                }
            }
            PreflopAction::Call => {
                let to_call = (state.to_call - state.invested[idx]).max(0.0);
                let call_amount = to_call.min(new_state.stacks[idx]);

                new_state.stacks[idx] -= call_amount;
                new_state.invested[idx] += call_amount;
                new_state.pot += call_amount;

                if new_state.stacks[idx] <= 0.001 {
                    new_state.all_in[idx] = true;
                }

                // Check if action is complete
                if new_state.is_action_complete() {
                    new_state.is_terminal = true;
                    new_state.to_act = None;
                } else {
                    new_state.to_act = new_state.next_to_act(pos);
                    if new_state.to_act.is_none() {
                        new_state.is_terminal = true;
                    }
                }

                // Track callers
                new_state.num_callers += 1;
            }
            PreflopAction::Raise(amount_centi) => {
                let raise_to = centi_to_bb(*amount_centi);
                let additional = (raise_to - new_state.invested[idx]).min(new_state.stacks[idx]);

                new_state.stacks[idx] -= additional;
                new_state.invested[idx] += additional;
                new_state.pot += additional;

                // Update betting info
                new_state.to_call = new_state.invested[idx];
                new_state.last_raise_size = raise_to - state.to_call;
                new_state.last_aggressor = Some(pos);
                new_state.bet_level = new_state.bet_level.next();
                new_state.num_callers = 0;

                // Reset has_acted for other players
                for i in 0..8 {
                    if i != idx && !new_state.folded[i] && !new_state.all_in[i] {
                        new_state.has_acted[i] = false;
                    }
                }

                // Find next player
                new_state.to_act = new_state.next_to_act(pos);
                if new_state.to_act.is_none() {
                    new_state.is_terminal = true;
                }
            }
            PreflopAction::AllIn => {
                let allin_amount = new_state.stacks[idx];
                new_state.stacks[idx] = 0.0;
                new_state.invested[idx] += allin_amount;
                new_state.pot += allin_amount;
                new_state.all_in[idx] = true;

                // Check if this is a raise
                if new_state.invested[idx] > state.to_call {
                    new_state.to_call = new_state.invested[idx];
                    new_state.last_aggressor = Some(pos);
                    new_state.bet_level = BetLevel::AllIn;
                    new_state.num_callers = 0;

                    // Reset has_acted for other players
                    for i in 0..8 {
                        if i != idx && !new_state.folded[i] && !new_state.all_in[i] {
                            new_state.has_acted[i] = false;
                        }
                    }
                }

                // Check if action complete or find next player
                if new_state.is_action_complete() {
                    new_state.is_terminal = true;
                    new_state.to_act = None;
                } else {
                    new_state.to_act = new_state.next_to_act(pos);
                    if new_state.to_act.is_none() {
                        new_state.is_terminal = true;
                    }
                }
            }
        }

        new_state
    }

    /// Calculate payoff for a player at a terminal state.
    fn calculate_payoff(&self, state: &PreflopState, player: usize) -> f64 {
        debug_assert!(state.is_terminal);

        // Check if player folded
        if state.folded[player] {
            return -state.invested[player];
        }

        // Count active players
        let active: Vec<usize> = (0..8)
            .filter(|&i| !state.folded[i])
            .collect();

        if active.len() == 1 {
            // Everyone else folded, player wins pot
            return state.pot - state.invested[player];
        }

        // Multiple players remain - use equity for expected value
        // This is the "equity realization" approach
        let player_class = state.hand_class.unwrap_or(84); // Default to middle strength

        // Calculate average equity vs opponents
        let mut total_equity = 0.0;
        let mut num_opponents = 0;

        for &opp in &active {
            if opp != player {
                // Assume average hand for opponent (simplified)
                // In real implementation, this would use range vs range equity
                let opp_class = 84; // Middle strength assumption
                let equity = self.equity_calc.equity_vs_hand(player_class, opp_class);
                total_equity += equity;
                num_opponents += 1;
            }
        }

        let avg_equity = if num_opponents > 0 {
            total_equity / num_opponents as f64
        } else {
            0.5
        };

        // For multiway, equity is lower
        let multiway_factor = if active.len() > 2 {
            1.0 / (active.len() - 1) as f64
        } else {
            1.0
        };

        let effective_equity = avg_equity * multiway_factor;

        // Expected value = equity * pot - invested
        (effective_equity * state.pot) - state.invested[player]
    }
}

impl Default for Preflop8MaxGame {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for Preflop8MaxGame {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Preflop8MaxGame")
            .field("stack_bb", &self.config.stack_bb)
            .finish()
    }
}

impl Game for Preflop8MaxGame {
    type State = PreflopState;
    type Action = PreflopAction;
    type InfoState = PreflopInfoState;

    fn initial_state(&self) -> Self::State {
        PreflopState::new(
            self.config.stack_bb,
            self.config.sb_amount,
            self.config.bb_amount,
            self.config.ante,
        )
    }

    fn is_terminal(&self, state: &Self::State) -> bool {
        state.is_terminal
    }

    fn get_payoff(&self, state: &Self::State, player: usize) -> f64 {
        self.calculate_payoff(state, player)
    }

    fn current_player(&self, state: &Self::State) -> Option<usize> {
        if self.is_terminal(state) || self.is_chance(state) {
            None
        } else {
            state.current_player()
        }
    }

    fn num_players(&self) -> usize {
        8
    }

    fn available_actions(&self, state: &Self::State) -> Vec<Self::Action> {
        if self.is_terminal(state) || self.is_chance(state) {
            return vec![];
        }
        self.get_available_actions(state)
    }

    fn apply_action(&self, state: &Self::State, action: &Self::Action) -> Self::State {
        self.apply_action(state, action)
    }

    fn info_state(&self, state: &Self::State) -> Self::InfoState {
        let pos = state.to_act.unwrap_or(Position8Max::UTG);
        let hand_class = state.hand_class.unwrap_or(0);

        PreflopInfoState {
            position: pos,
            hand_class,
            history: state.action_history.clone(),
        }
    }

    fn is_chance(&self, state: &Self::State) -> bool {
        // Need to deal cards if hand_class is not set
        state.hand_class.is_none() && state.to_act.is_some()
    }

    fn sample_chance<R: Rng>(&self, state: &Self::State, rng: &mut R) -> Self::State {
        let mut new_state = state.clone();

        // Sample a hand class (0-168) weighted by number of combos
        let hand_class = sample_hand_class(rng);
        new_state.hand_class = Some(hand_class);

        new_state
    }

    fn action_name(&self, action: &Self::Action) -> String {
        format!("{}", action)
    }

    fn state_description(&self, state: &Self::State) -> String {
        format!("{:?}", state)
    }
}

/// Sample a random hand class weighted by number of combos.
fn sample_hand_class<R: Rng>(rng: &mut R) -> u8 {
    use crate::games::preflop::abstraction::HandClass;

    // Total combos: 1326
    let roll: u32 = rng.gen_range(0..1326);
    let mut cumsum = 0u32;

    for class_idx in 0..169u8 {
        let hc = HandClass::from_index(class_idx);
        cumsum += hc.num_combos() as u32;
        if roll < cumsum {
            return class_idx;
        }
    }

    168 // Fallback (shouldn't reach)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_initial_state() {
        let game = Preflop8MaxGame::new();
        let state = game.initial_state();

        assert!(!game.is_terminal(&state));
        assert!(game.is_chance(&state)); // Need to deal cards
        assert_eq!(state.to_act, Some(Position8Max::UTG));
    }

    #[test]
    fn test_deal_and_act() {
        let game = Preflop8MaxGame::new();
        let mut rng = rand::thread_rng();

        let state = game.initial_state();
        let state = game.sample_chance(&state, &mut rng);

        assert!(!game.is_chance(&state));
        assert!(state.hand_class.is_some());

        let actions = game.available_actions(&state);
        assert!(!actions.is_empty());

        // Should have fold, call, raise options
        assert!(actions.iter().any(|a| matches!(a, PreflopAction::Fold)));
        assert!(actions.iter().any(|a| matches!(a, PreflopAction::Call)));
    }

    #[test]
    fn test_fold_terminates() {
        let game = Preflop8MaxGame::new();
        let mut rng = rand::thread_rng();

        let mut state = game.initial_state();
        state = game.sample_chance(&state, &mut rng);

        // All players fold except BB
        for _ in 0..7 {
            if !game.is_terminal(&state) && state.to_act.is_some() {
                state = game.apply_action(&state, &PreflopAction::Fold);
            }
        }

        // Should be terminal
        assert!(game.is_terminal(&state));
    }

    #[test]
    fn test_raise_chain() {
        let game = Preflop8MaxGame::new();
        let mut rng = rand::thread_rng();

        let mut state = game.initial_state();
        state = game.sample_chance(&state, &mut rng);

        // UTG raises
        let actions = game.available_actions(&state);
        let raise = actions.iter()
            .find(|a| matches!(a, PreflopAction::Raise(_)))
            .cloned()
            .unwrap();
        state = game.apply_action(&state, &raise);

        // Check bet level advanced
        assert_eq!(state.bet_level, BetLevel::FacingRaise);
        assert_eq!(state.last_aggressor, Some(Position8Max::UTG));
    }

    #[test]
    fn test_payoff_calculation() {
        let game = Preflop8MaxGame::new();
        let mut rng = rand::thread_rng();

        let mut state = game.initial_state();
        state = game.sample_chance(&state, &mut rng);

        // Everyone folds to BB
        for _ in 0..7 {
            if !game.is_terminal(&state) && state.to_act.is_some() {
                state = game.apply_action(&state, &PreflopAction::Fold);
            }
        }

        // BB should win the pot
        let bb_payoff = game.get_payoff(&state, Position8Max::BB.index());
        assert!(bb_payoff > 0.0, "BB should profit when everyone folds");

        // Others should lose their investment
        let utg_payoff = game.get_payoff(&state, Position8Max::UTG.index());
        assert!(utg_payoff < 0.0, "UTG should lose ante when folding");
    }
}
