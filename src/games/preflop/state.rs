//! Poker game state representation.
//!
//! This module defines the complete state of a poker hand, including
//! hole cards, board cards, betting history, and pot/stack information.

use super::card::{HoleCards, Board, Deck, Street};
use super::action::PokerAction;
use crate::cfr::game::GameState;
use std::fmt;

/// Position in a heads-up poker game.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HUPosition {
    /// Small blind (out of position postflop, acts first preflop except for last action)
    SB = 0,
    /// Big blind (in position postflop)
    BB = 1,
}

impl HUPosition {
    /// Get the other position.
    pub fn opponent(&self) -> Self {
        match self {
            HUPosition::SB => HUPosition::BB,
            HUPosition::BB => HUPosition::SB,
        }
    }

    /// Get position index (0 or 1).
    pub fn index(&self) -> usize {
        *self as usize
    }

    /// Get position from index.
    pub fn from_index(idx: usize) -> Self {
        match idx {
            0 => HUPosition::SB,
            _ => HUPosition::BB,
        }
    }

    /// Check if this position is in position postflop.
    pub fn is_ip_postflop(&self) -> bool {
        matches!(self, HUPosition::BB)
    }

    /// Get the position that acts first on a given street.
    pub fn first_to_act(street: Street) -> Self {
        match street {
            Street::Preflop => HUPosition::SB,
            _ => HUPosition::SB, // OOP acts first postflop
        }
    }
}

impl fmt::Display for HUPosition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HUPosition::SB => write!(f, "SB"),
            HUPosition::BB => write!(f, "BB"),
        }
    }
}

/// Complete state of a poker hand.
#[derive(Clone)]
pub struct PokerState {
    /// Hole cards for each player [SB, BB]
    pub hands: [Option<HoleCards>; 2],
    /// Community cards
    pub board: Board,
    /// Deck for dealing remaining cards
    pub deck: Deck,

    /// Current street
    pub street: Street,
    /// Current pot (total money in the middle)
    pub pot: f64,
    /// Stack sizes for each player [SB, BB]
    pub stacks: [f64; 2],
    /// Amount invested in the current street [SB, BB]
    pub invested_street: [f64; 2],
    /// Total amount invested in the hand [SB, BB]
    pub invested_total: [f64; 2],

    /// Amount to call (0 if check is available)
    pub to_call: f64,
    /// Minimum raise amount
    pub min_raise: f64,
    /// Last aggressor's bet/raise amount
    pub last_bet_size: f64,

    /// Player to act next (None if terminal or chance)
    pub to_act: Option<HUPosition>,
    /// Number of bets/raises on the current street
    pub num_bets_street: u8,
    /// Number of actions on the current street
    pub num_actions_street: u8,

    /// Action history for the entire hand
    pub action_history: Vec<PokerAction>,
    /// Action history keys per street (for info state generation)
    pub street_history: Vec<String>,

    /// Whether this is a terminal state
    pub is_terminal: bool,
    /// Whether someone folded
    pub folded: Option<HUPosition>,
    /// Whether someone is all-in
    pub all_in: [bool; 2],
}

impl PokerState {
    /// Create a new initial state for a heads-up hand.
    pub fn new_hu(starting_stacks: [f64; 2], sb_amount: f64, bb_amount: f64) -> Self {
        Self {
            hands: [None, None],
            board: Board::new(),
            deck: Deck::new(),

            street: Street::Preflop,
            pot: sb_amount + bb_amount,
            stacks: [
                starting_stacks[0] - sb_amount,
                starting_stacks[1] - bb_amount,
            ],
            invested_street: [sb_amount, bb_amount],
            invested_total: [sb_amount, bb_amount],

            to_call: bb_amount - sb_amount, // SB needs to call 0.5bb more
            min_raise: bb_amount, // Minimum raise is 1bb (raise by bb amount)
            last_bet_size: bb_amount,

            to_act: None, // Needs card dealing first
            num_bets_street: 1, // BB counts as first "bet"
            num_actions_street: 0,

            action_history: Vec::new(),
            street_history: vec![String::new()],

            is_terminal: false,
            folded: None,
            all_in: [false, false],
        }
    }

    /// Create state after dealing hole cards.
    pub fn with_hands(mut self, sb_hand: HoleCards, bb_hand: HoleCards) -> Self {
        self.hands = [Some(sb_hand), Some(bb_hand)];
        self.to_act = Some(HUPosition::SB); // SB acts first preflop
        self
    }

    /// Get the hole cards for a player.
    pub fn hand(&self, pos: HUPosition) -> Option<&HoleCards> {
        self.hands[pos.index()].as_ref()
    }

    /// Get the current player to act.
    pub fn current_player(&self) -> Option<HUPosition> {
        self.to_act
    }

    /// Get the current player's stack.
    pub fn current_stack(&self) -> f64 {
        match self.to_act {
            Some(pos) => self.stacks[pos.index()],
            None => 0.0,
        }
    }

    /// Get effective stack (minimum of both stacks).
    pub fn effective_stack(&self) -> f64 {
        self.stacks[0].min(self.stacks[1])
    }

    /// Get stack-to-pot ratio.
    pub fn spr(&self) -> f64 {
        self.effective_stack() / self.pot
    }

    /// Check if current player can check.
    pub fn can_check(&self) -> bool {
        self.to_call == 0.0
    }

    /// Check if the hand is complete.
    pub fn is_complete(&self) -> bool {
        self.is_terminal
    }

    /// Check if we're at a showdown.
    pub fn is_showdown(&self) -> bool {
        self.is_terminal && self.folded.is_none()
    }

    /// Get the winner if someone folded.
    pub fn fold_winner(&self) -> Option<HUPosition> {
        self.folded.map(|f| f.opponent())
    }

    /// Check if both players are all-in.
    pub fn both_all_in(&self) -> bool {
        self.all_in[0] && self.all_in[1]
    }

    /// Generate an action history string for the current street.
    pub fn street_action_string(&self) -> String {
        if self.street_history.is_empty() {
            String::new()
        } else {
            self.street_history.last().cloned().unwrap_or_default()
        }
    }

    /// Generate a full action history string.
    pub fn full_history_string(&self) -> String {
        self.street_history.join("|")
    }

    /// Apply an action to this state, returning a new state.
    pub fn apply(&self, action: PokerAction) -> Self {
        let mut new_state = self.clone();
        new_state.apply_action_mut(action);
        new_state
    }

    /// Apply an action mutably.
    fn apply_action_mut(&mut self, action: PokerAction) {
        let pos = self.to_act.expect("No player to act");
        let idx = pos.index();

        // Record action
        self.action_history.push(action);
        if let Some(last) = self.street_history.last_mut() {
            if !last.is_empty() {
                last.push('-');
            }
            last.push_str(&action.short_code());
        }
        self.num_actions_street += 1;

        match action {
            PokerAction::Fold => {
                self.folded = Some(pos);
                self.is_terminal = true;
                self.to_act = None;
            }
            PokerAction::Check => {
                // Check if street is complete
                if self.is_street_complete_after_check(pos) {
                    self.advance_street();
                } else {
                    self.to_act = Some(pos.opponent());
                }
            }
            PokerAction::Call => {
                let call_amount = self.to_call.min(self.stacks[idx]);
                self.stacks[idx] -= call_amount;
                self.pot += call_amount;
                self.invested_street[idx] += call_amount;
                self.invested_total[idx] += call_amount;

                if self.stacks[idx] <= 0.0 {
                    self.all_in[idx] = true;
                }

                self.to_call = 0.0;

                // Determine if the street is complete after this call
                let street_complete = if self.both_all_in() {
                    true
                } else if self.street == Street::Preflop {
                    // Preflop special case: SB limping doesn't close the action
                    // BB still has the option to check or raise
                    // Street is complete when BB calls a raise, or after BB's option
                    pos == HUPosition::BB || (pos == HUPosition::SB && self.num_bets_street > 1)
                } else {
                    // Postflop: a call always closes the action on this round
                    true
                };

                if street_complete {
                    if self.street == Street::River {
                        self.is_terminal = true;
                        self.to_act = None;
                    } else {
                        self.advance_street();
                    }
                } else {
                    // SB limped preflop, BB gets to act
                    self.to_act = Some(HUPosition::BB);
                }
            }
            PokerAction::Bet(amount_centi) => {
                let amount = amount_centi as f64 / 100.0;
                let bet_amount = amount.min(self.stacks[idx]);

                self.stacks[idx] -= bet_amount;
                self.pot += bet_amount;
                self.invested_street[idx] += bet_amount;
                self.invested_total[idx] += bet_amount;

                self.to_call = bet_amount;
                self.min_raise = bet_amount * 2.0; // Min raise is 2x the bet
                self.last_bet_size = bet_amount;
                self.num_bets_street += 1;

                if self.stacks[idx] <= 0.0 {
                    self.all_in[idx] = true;
                }

                self.to_act = Some(pos.opponent());
            }
            PokerAction::Raise(amount_centi) => {
                let raise_to = amount_centi as f64 / 100.0;
                let additional = (raise_to - self.invested_street[idx]).min(self.stacks[idx]);

                self.stacks[idx] -= additional;
                self.pot += additional;
                self.invested_street[idx] += additional;
                self.invested_total[idx] += additional;

                let raise_size = self.invested_street[idx] - self.invested_street[pos.opponent().index()];
                self.to_call = raise_size;
                self.min_raise = self.invested_street[idx] + raise_size; // Min 3bet is 2x raise
                self.last_bet_size = raise_size;
                self.num_bets_street += 1;

                if self.stacks[idx] <= 0.0 {
                    self.all_in[idx] = true;
                }

                self.to_act = Some(pos.opponent());
            }
            PokerAction::AllIn => {
                let allin_amount = self.stacks[idx];
                self.stacks[idx] = 0.0;
                self.pot += allin_amount;
                self.invested_street[idx] += allin_amount;
                self.invested_total[idx] += allin_amount;
                self.all_in[idx] = true;

                // Determine if this is a bet or raise based on to_call
                if self.to_call == 0.0 {
                    // This is an all-in bet
                    self.to_call = allin_amount;
                    self.last_bet_size = allin_amount;
                } else {
                    // This is an all-in raise
                    let raise_size = allin_amount + self.invested_street[idx] - self.invested_street[pos.opponent().index()] - self.to_call;
                    self.to_call = (self.invested_street[idx] - self.invested_street[pos.opponent().index()]).max(0.0);
                    self.last_bet_size = raise_size.max(0.0);
                }
                self.num_bets_street += 1;

                // Check if opponent can still act
                if self.to_call > 0.0 && !self.all_in[pos.opponent().index()] {
                    self.to_act = Some(pos.opponent());
                } else {
                    // Hand is over, go to showdown
                    self.is_terminal = true;
                    self.to_act = None;
                }
            }
        }
    }

    /// Check if the street is complete after a check.
    fn is_street_complete_after_check(&self, checker: HUPosition) -> bool {
        match self.street {
            Street::Preflop => {
                // BB checking preflop ends the street if SB limped
                // After SB limp (1 action), BB check (2 actions) ends the street
                checker == HUPosition::BB && self.num_actions_street >= 2
            }
            _ => {
                // Postflop: check-check ends the street (need 2 checks)
                // First check: num_actions_street = 1, second check: num_actions_street = 2
                self.num_actions_street >= 2
            }
        }
    }

    /// Advance to the next street.
    fn advance_street(&mut self) {
        if let Some(next) = self.street.next() {
            if next == Street::Showdown {
                self.is_terminal = true;
                self.to_act = None;
            } else {
                self.street = next;
                self.to_call = 0.0;
                self.min_raise = 1.0; // Reset to 1bb min bet
                self.last_bet_size = 0.0;
                self.num_bets_street = 0;
                self.num_actions_street = 0;
                self.invested_street = [0.0, 0.0];
                self.street_history.push(String::new());

                // OOP (SB) acts first postflop
                // But if someone is all-in, go to showdown
                if self.both_all_in() {
                    self.is_terminal = true;
                    self.to_act = None;
                } else if self.all_in[HUPosition::SB.index()] {
                    self.to_act = Some(HUPosition::BB);
                } else if self.all_in[HUPosition::BB.index()] {
                    self.to_act = Some(HUPosition::SB);
                } else {
                    self.to_act = Some(HUPosition::SB);
                }
            }
        } else {
            self.is_terminal = true;
            self.to_act = None;
        }
    }

    /// Deal the flop.
    pub fn deal_flop(&mut self) {
        debug_assert_eq!(self.street, Street::Flop);
        debug_assert_eq!(self.board.len(), 0);

        let cards = self.deck.deal_n(3);
        for card in cards {
            self.board.add(card);
        }
    }

    /// Deal the turn.
    pub fn deal_turn(&mut self) {
        debug_assert_eq!(self.street, Street::Turn);
        debug_assert_eq!(self.board.len(), 3);

        if let Some(card) = self.deck.deal() {
            self.board.add(card);
        }
    }

    /// Deal the river.
    pub fn deal_river(&mut self) {
        debug_assert_eq!(self.street, Street::River);
        debug_assert_eq!(self.board.len(), 4);

        if let Some(card) = self.deck.deal() {
            self.board.add(card);
        }
    }
}

impl GameState for PokerState {}

impl fmt::Debug for PokerState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "PokerState {{ ")?;
        write!(f, "street: {:?}, ", self.street)?;
        write!(f, "pot: {:.2}, ", self.pot)?;
        write!(f, "stacks: [{:.2}, {:.2}], ", self.stacks[0], self.stacks[1])?;
        write!(f, "to_act: {:?}, ", self.to_act)?;
        if let Some(h) = &self.hands[0] {
            write!(f, "SB: {}, ", h)?;
        }
        if let Some(h) = &self.hands[1] {
            write!(f, "BB: {}, ", h)?;
        }
        write!(f, "board: {}", self.board)?;
        write!(f, " }}")
    }
}

impl fmt::Display for PokerState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Street: {} | Pot: {:.2}bb", self.street, self.pot)?;
        writeln!(f, "Board: {}", if self.board.is_empty() { "none".to_string() } else { self.board.to_string() })?;
        writeln!(f, "SB: stack={:.2}bb, invested={:.2}bb{}",
            self.stacks[0], self.invested_total[0],
            if self.all_in[0] { " (all-in)" } else { "" })?;
        writeln!(f, "BB: stack={:.2}bb, invested={:.2}bb{}",
            self.stacks[1], self.invested_total[1],
            if self.all_in[1] { " (all-in)" } else { "" })?;
        if let Some(pos) = self.to_act {
            writeln!(f, "To act: {} (to_call: {:.2}bb)", pos, self.to_call)?;
        } else if let Some(folder) = self.folded {
            writeln!(f, "{} folded, {} wins", folder, folder.opponent())?;
        } else if self.is_terminal {
            writeln!(f, "Showdown")?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_initial_state() {
        let state = PokerState::new_hu([50.0, 50.0], 0.5, 1.0);

        assert_eq!(state.pot, 1.5);
        assert_eq!(state.stacks[0], 49.5); // SB posted
        assert_eq!(state.stacks[1], 49.0); // BB posted
        assert_eq!(state.to_call, 0.5); // SB needs to call 0.5 more
        assert_eq!(state.street, Street::Preflop);
        assert!(!state.is_terminal);
    }

    #[test]
    fn test_sb_fold() {
        let sb_hand = HoleCards::from_str("2c3d").unwrap();
        let bb_hand = HoleCards::from_str("AhKs").unwrap();

        let state = PokerState::new_hu([50.0, 50.0], 0.5, 1.0)
            .with_hands(sb_hand, bb_hand);

        assert_eq!(state.to_act, Some(HUPosition::SB));

        let new_state = state.apply(PokerAction::Fold);
        assert!(new_state.is_terminal);
        assert_eq!(new_state.folded, Some(HUPosition::SB));
        assert_eq!(new_state.fold_winner(), Some(HUPosition::BB));
    }

    #[test]
    fn test_limp_check() {
        let sb_hand = HoleCards::from_str("AsAd").unwrap();
        let bb_hand = HoleCards::from_str("KhKs").unwrap();

        let state = PokerState::new_hu([50.0, 50.0], 0.5, 1.0)
            .with_hands(sb_hand, bb_hand);

        // SB limps
        let state = state.apply(PokerAction::Call);
        assert_eq!(state.pot, 2.0);
        assert_eq!(state.stacks[0], 49.0);
        assert_eq!(state.to_act, Some(HUPosition::BB));

        // BB checks
        let state = state.apply(PokerAction::Check);
        assert_eq!(state.street, Street::Flop);
        assert!(!state.is_terminal);
    }

    #[test]
    fn test_raise_fold() {
        let sb_hand = HoleCards::from_str("AsAd").unwrap();
        let bb_hand = HoleCards::from_str("7c2d").unwrap();

        let state = PokerState::new_hu([50.0, 50.0], 0.5, 1.0)
            .with_hands(sb_hand, bb_hand);

        // SB raises to 3bb
        let state = state.apply(PokerAction::Raise(300));
        assert_eq!(state.to_act, Some(HUPosition::BB));
        assert!(state.to_call > 0.0);

        // BB folds
        let state = state.apply(PokerAction::Fold);
        assert!(state.is_terminal);
        assert_eq!(state.fold_winner(), Some(HUPosition::SB));
    }

    #[test]
    fn test_all_in_preflop() {
        let sb_hand = HoleCards::from_str("AsAd").unwrap();
        let bb_hand = HoleCards::from_str("KhKs").unwrap();

        let state = PokerState::new_hu([50.0, 50.0], 0.5, 1.0)
            .with_hands(sb_hand, bb_hand);

        // SB all-in
        let state = state.apply(PokerAction::AllIn);
        assert!(state.all_in[0]);
        assert_eq!(state.stacks[0], 0.0);

        // BB calls all-in
        let state = state.apply(PokerAction::Call);
        assert!(state.is_terminal);
        assert!(state.both_all_in());
        assert!(state.is_showdown());
    }

    #[test]
    fn test_postflop_betting() {
        let sb_hand = HoleCards::from_str("AsAd").unwrap();
        let bb_hand = HoleCards::from_str("KhKs").unwrap();

        let mut state = PokerState::new_hu([50.0, 50.0], 0.5, 1.0)
            .with_hands(sb_hand, bb_hand);

        // SB limps, BB checks to flop
        state = state.apply(PokerAction::Call);
        state = state.apply(PokerAction::Check);

        assert_eq!(state.street, Street::Flop);
        assert_eq!(state.to_act, Some(HUPosition::SB));

        // SB bets 66% pot
        let bet_size = (state.pot * 0.66 * 100.0).round() as u32;
        state = state.apply(PokerAction::Bet(bet_size));

        assert!(state.to_call > 0.0);
        assert_eq!(state.to_act, Some(HUPosition::BB));

        // BB calls
        state = state.apply(PokerAction::Call);
        assert_eq!(state.street, Street::Turn);
    }

    #[test]
    fn test_action_history() {
        let sb_hand = HoleCards::from_str("AsAd").unwrap();
        let bb_hand = HoleCards::from_str("KhKs").unwrap();

        let mut state = PokerState::new_hu([50.0, 50.0], 0.5, 1.0)
            .with_hands(sb_hand, bb_hand);

        state = state.apply(PokerAction::Raise(300)); // SB raises
        state = state.apply(PokerAction::Call); // BB calls

        let history = state.full_history_string();
        assert!(history.contains("R300"));
        assert!(history.contains("C"));
    }
}
