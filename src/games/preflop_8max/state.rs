//! Preflop game state for 8-max tables.

use crate::cfr::game::GameState;
use std::fmt;

/// Position in an 8-max poker game.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Position8Max {
    UTG = 0,
    EP = 1,
    MP = 2,
    HJ = 3,
    CO = 4,
    BU = 5,
    SB = 6,
    BB = 7,
}

impl Position8Max {
    /// All positions in preflop action order.
    pub const ALL: [Position8Max; 8] = [
        Position8Max::UTG,
        Position8Max::EP,
        Position8Max::MP,
        Position8Max::HJ,
        Position8Max::CO,
        Position8Max::BU,
        Position8Max::SB,
        Position8Max::BB,
    ];

    /// Get position from index.
    pub fn from_index(idx: usize) -> Option<Self> {
        match idx {
            0 => Some(Position8Max::UTG),
            1 => Some(Position8Max::EP),
            2 => Some(Position8Max::MP),
            3 => Some(Position8Max::HJ),
            4 => Some(Position8Max::CO),
            5 => Some(Position8Max::BU),
            6 => Some(Position8Max::SB),
            7 => Some(Position8Max::BB),
            _ => None,
        }
    }

    /// Get position index.
    pub fn index(&self) -> usize {
        *self as usize
    }

    /// Get position name.
    pub fn name(&self) -> &'static str {
        match self {
            Position8Max::UTG => "UTG",
            Position8Max::EP => "EP",
            Position8Max::MP => "MP",
            Position8Max::HJ => "HJ",
            Position8Max::CO => "CO",
            Position8Max::BU => "BU",
            Position8Max::SB => "SB",
            Position8Max::BB => "BB",
        }
    }

    /// Check if this position is in position vs another (postflop).
    pub fn is_ip_vs(&self, other: &Position8Max) -> bool {
        self.index() > other.index()
    }

    /// Check if this is a blind position.
    pub fn is_blind(&self) -> bool {
        matches!(self, Position8Max::SB | Position8Max::BB)
    }

    /// Get the next position in action order.
    pub fn next(&self) -> Option<Position8Max> {
        Position8Max::from_index(self.index() + 1)
    }
}

impl fmt::Display for Position8Max {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name())
    }
}

/// Betting level in preflop action.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BetLevel {
    /// No action yet (unopened pot)
    Unopened,
    /// Facing a raise (RFI) - can fold/call/3bet
    FacingRaise,
    /// Facing a 3-bet - can fold/call/4bet
    Facing3Bet,
    /// Facing a 4-bet - can fold/call/5bet
    Facing4Bet,
    /// Facing a 5-bet - can fold/call/allin
    Facing5Bet,
    /// All-in situation
    AllIn,
}

impl BetLevel {
    /// Get the next bet level after a raise.
    pub fn next(&self) -> Self {
        match self {
            BetLevel::Unopened => BetLevel::FacingRaise,
            BetLevel::FacingRaise => BetLevel::Facing3Bet,
            BetLevel::Facing3Bet => BetLevel::Facing4Bet,
            BetLevel::Facing4Bet => BetLevel::Facing5Bet,
            BetLevel::Facing5Bet | BetLevel::AllIn => BetLevel::AllIn,
        }
    }

    /// Get index for flat counting (0=RFI, 1=facing RFI, etc.).
    pub fn flat_index(&self) -> usize {
        match self {
            BetLevel::Unopened => 0,
            BetLevel::FacingRaise => 1,
            BetLevel::Facing3Bet => 2,
            BetLevel::Facing4Bet => 3,
            BetLevel::Facing5Bet | BetLevel::AllIn => 4,
        }
    }
}

/// State of a preflop hand for 8-max.
#[derive(Clone)]
pub struct PreflopState {
    /// Stack sizes for each position (in BB).
    pub stacks: [f64; 8],
    /// Amount invested by each position this hand.
    pub invested: [f64; 8],
    /// Whether each position has folded.
    pub folded: [bool; 8],
    /// Whether each position is all-in.
    pub all_in: [bool; 8],
    /// Whether each position has acted this round.
    pub has_acted: [bool; 8],

    /// Current pot size.
    pub pot: f64,
    /// Amount to call.
    pub to_call: f64,
    /// Last raise size (for min-raise calculation).
    pub last_raise_size: f64,

    /// Current betting level.
    pub bet_level: BetLevel,
    /// Position of the last aggressor.
    pub last_aggressor: Option<Position8Max>,
    /// Number of callers at current bet level.
    pub num_callers: u8,

    /// Position to act next.
    pub to_act: Option<Position8Max>,
    /// Whether hand is complete.
    pub is_terminal: bool,

    /// Action history string for info state.
    pub action_history: String,

    /// Hand class for the acting player (0-168).
    pub hand_class: Option<u8>,

    /// SB amount.
    pub sb_amount: f64,
    /// BB amount.
    pub bb_amount: f64,
    /// Ante per player.
    pub ante: f64,
}

impl PreflopState {
    /// Create a new initial state.
    pub fn new(
        stack_bb: f64,
        sb_amount: f64,
        bb_amount: f64,
        ante: f64,
    ) -> Self {
        let mut stacks = [stack_bb; 8];
        let mut invested = [0.0; 8];

        // Post blinds
        stacks[Position8Max::SB.index()] -= sb_amount;
        invested[Position8Max::SB.index()] = sb_amount;

        stacks[Position8Max::BB.index()] -= bb_amount;
        invested[Position8Max::BB.index()] = bb_amount;

        // Post antes
        let mut pot = sb_amount + bb_amount;
        for i in 0..8 {
            stacks[i] -= ante;
            invested[i] += ante;
            pot += ante;
        }

        Self {
            stacks,
            invested,
            folded: [false; 8],
            all_in: [false; 8],
            has_acted: [false; 8],
            pot,
            to_call: bb_amount,
            last_raise_size: bb_amount,
            bet_level: BetLevel::Unopened,
            last_aggressor: None,
            num_callers: 0,
            to_act: Some(Position8Max::UTG),
            is_terminal: false,
            action_history: String::new(),
            hand_class: None,
            sb_amount,
            bb_amount,
            ante,
        }
    }

    /// Get the current player index (for CFR).
    pub fn current_player(&self) -> Option<usize> {
        self.to_act.map(|p| p.index())
    }

    /// Get effective stack (smallest stack among active players).
    pub fn effective_stack(&self) -> f64 {
        Position8Max::ALL.iter()
            .filter(|p| !self.folded[p.index()])
            .map(|p| self.stacks[p.index()])
            .fold(f64::INFINITY, f64::min)
    }

    /// Count active players (not folded).
    pub fn active_players(&self) -> usize {
        self.folded.iter().filter(|&&f| !f).count()
    }

    /// Count players who have put money in (invested > ante).
    pub fn players_in_pot(&self) -> usize {
        self.invested.iter()
            .zip(self.folded.iter())
            .filter(|(&inv, &folded)| !folded && inv > self.ante)
            .count()
    }

    /// Check if this is a heads-up situation.
    pub fn is_heads_up(&self) -> bool {
        self.active_players() == 2
    }

    /// Get the next position to act after current position.
    pub fn next_to_act(&self, current: Position8Max) -> Option<Position8Max> {
        let start = current.index() + 1;

        // First, check positions after current
        for i in start..8 {
            if !self.folded[i] && !self.all_in[i] {
                return Position8Max::from_index(i);
            }
        }

        // Then wrap around to earlier positions (for BB option, etc.)
        for i in 0..current.index() {
            if !self.folded[i] && !self.all_in[i] {
                // Only if they haven't acted yet this round
                if !self.has_acted[i] || (self.to_call > self.invested[i] - self.invested[current.index()].max(0.0)) {
                    return Position8Max::from_index(i);
                }
            }
        }

        None
    }

    /// Check if action is complete (everyone has acted and amounts are equal).
    pub fn is_action_complete(&self) -> bool {
        if self.active_players() <= 1 {
            return true;
        }

        // Find the highest investment among active players
        let max_invested = self.invested.iter()
            .enumerate()
            .filter(|(i, _)| !self.folded[*i])
            .map(|(_, &v)| v)
            .fold(0.0, f64::max);

        // All active players must have acted and matched the highest investment
        for i in 0..8 {
            if !self.folded[i] && !self.all_in[i] {
                // Must have acted and matched (or be all-in)
                if !self.has_acted[i] {
                    return false;
                }
                if (self.invested[i] - max_invested).abs() > 0.001 {
                    return false;
                }
            }
        }

        true
    }

    /// Get SPR (stack to pot ratio) for effective stack.
    pub fn spr(&self) -> f64 {
        self.effective_stack() / self.pot
    }
}

impl GameState for PreflopState {}

impl fmt::Debug for PreflopState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "PreflopState {{ pot: {:.2}, to_call: {:.2}, to_act: {:?}, level: {:?} }}",
            self.pot, self.to_call, self.to_act, self.bet_level)
    }
}

impl fmt::Display for PreflopState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Pot: {:.2}bb | To call: {:.2}bb | Level: {:?}",
            self.pot, self.to_call, self.bet_level)?;

        for pos in Position8Max::ALL.iter() {
            let idx = pos.index();
            let status = if self.folded[idx] {
                "folded"
            } else if self.all_in[idx] {
                "all-in"
            } else {
                "active"
            };
            writeln!(f, "{}: stack={:.2}bb invested={:.2}bb ({})",
                pos.name(), self.stacks[idx], self.invested[idx], status)?;
        }

        if let Some(actor) = self.to_act {
            writeln!(f, "To act: {}", actor)?;
        } else {
            writeln!(f, "Action complete")?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_initial_state() {
        let state = PreflopState::new(50.0, 0.5, 1.0, 0.12);

        // Check blinds posted
        assert!((state.invested[Position8Max::SB.index()] - 0.62).abs() < 0.01);
        assert!((state.invested[Position8Max::BB.index()] - 1.12).abs() < 0.01);

        // Check pot
        let expected_pot = 0.5 + 1.0 + (0.12 * 8.0);
        assert!((state.pot - expected_pot).abs() < 0.01);

        // UTG acts first
        assert_eq!(state.to_act, Some(Position8Max::UTG));
    }

    #[test]
    fn test_position_ordering() {
        assert!(Position8Max::BB.is_ip_vs(&Position8Max::SB));
        assert!(Position8Max::BU.is_ip_vs(&Position8Max::CO));
        assert!(!Position8Max::UTG.is_ip_vs(&Position8Max::BB));
    }

    #[test]
    fn test_bet_level_progression() {
        assert_eq!(BetLevel::Unopened.next(), BetLevel::FacingRaise);
        assert_eq!(BetLevel::FacingRaise.next(), BetLevel::Facing3Bet);
        assert_eq!(BetLevel::Facing3Bet.next(), BetLevel::Facing4Bet);
        assert_eq!(BetLevel::Facing4Bet.next(), BetLevel::Facing5Bet);
        assert_eq!(BetLevel::Facing5Bet.next(), BetLevel::AllIn);
    }
}
