//! Preflop poker game tree implementation.
//!
//! This module provides structures for building and solving preflop
//! poker game trees using CFR, including full postflop play.
//!
//! ## Modules
//!
//! - `card`: Card, hand, board, and deck representations
//! - `hand`: Hand ranges and combo enumeration
//! - `hand_eval`: Poker hand evaluation
//! - `abstraction`: Card abstraction for state bucketing
//! - `action`: Poker betting actions
//! - `state`: Complete game state
//! - `betting`: Betting logic and action generation
//! - `info_state`: Information state for CFR
//! - `game`: SB vs BB full game implementation
//! - `config`: Preflop-only configuration
//! - `postflop_config`: Full game configuration
//! - `output`: Solution export utilities

pub mod card;
pub mod hand;
pub mod hand_eval;
pub mod abstraction;
pub mod action;
pub mod state;
pub mod betting;
pub mod info_state;
pub mod game;
pub mod config;
pub mod postflop_config;
pub mod output;

// Re-export commonly used types
pub use card::{Card, HoleCards, Board, Deck, Street};
pub use hand::Range;
pub use hand_eval::HandEvaluator;
pub use abstraction::{CardAbstraction, AbstractionConfig, HandClass};
pub use action::PokerAction;
pub use state::{PokerState, HUPosition};
pub use betting::{BettingLogic, BettingConfig};
pub use info_state::PokerInfoState;
pub use game::{SBvsBBFullGame, SBvsBBConfig};
pub use config::*;
pub use postflop_config::FullGameConfig;
