//! 8-max preflop solver module.
//!
//! This module implements a preflop-only poker solver for 8-max tables.
//! Instead of solving full postflop trees, it uses equity realization
//! to estimate postflop value, enabling fast convergence.
//!
//! Key features:
//! - Supports all 8 positions (UTG, EP, MP, HJ, CO, BU, SB, BB)
//! - Configurable bet sizing (open, 3bet, 4bet, 5bet)
//! - Equity-based postflop value estimation
//! - HRC-compatible settings

mod state;
mod action;
mod game;
mod equity;

pub use state::{PreflopState, Position8Max};
pub use action::PreflopAction;
pub use game::{Preflop8MaxGame, Preflop8MaxConfig};
pub use equity::EquityCalculator;
