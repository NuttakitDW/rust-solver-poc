//! Game implementations for the CFR solver.
//!
//! This module contains implementations of various games that can be solved
//! using the generic CFR solver. These serve as:
//!
//! 1. **Validation**: Games with known Nash equilibria (like Kuhn Poker) verify
//!    that the CFR implementation is correct.
//!
//! 2. **Examples**: Demonstrate how to implement the `Game` trait for new games.
//!
//! 3. **Benchmarks**: Provide standardized games for performance testing.
//!
//! ## Available Games
//!
//! - [`kuhn`]: Kuhn Poker - A simplified 3-card poker game with known Nash equilibrium
//! - [`preflop`]: Texas Hold'em preflop solver (planned)
//!
//! ## Adding New Games
//!
//! To add a new game:
//!
//! 1. Create a new module under `src/games/`
//! 2. Define state, action, and info state types
//! 3. Implement the `Game` trait
//! 4. Add tests that verify expected behavior
//!
//! See the [`kuhn`] module for a complete example.

pub mod kuhn;
pub mod preflop;
pub mod preflop_8max;
pub mod preflop_ranges;
