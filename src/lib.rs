//! # Rust Solver POC
//!
//! A generic Counterfactual Regret Minimization (CFR) solver for computing
//! Nash equilibrium strategies in extensive-form games.
//!
//! ## Features
//!
//! - **Generic CFR Engine**: Works with any game implementing the `Game` trait
//! - **Multiple Variants**: Supports CFR+, Linear CFR, MCCFR, and Discounted CFR
//! - **Thread-Safe Storage**: Designed for future parallel implementations
//! - **Checkpointing**: Save and resume solver state
//! - **Exploitability Calculation**: Measure strategy quality
//!
//! ## Quick Start
//!
//! ```ignore
//! use rust_solver_poc::cfr::{Game, CFRSolver, CFRConfig};
//!
//! // 1. Implement the Game trait for your game
//! // 2. Create a solver
//! let solver = CFRSolver::new(my_game, CFRConfig::default());
//!
//! // 3. Train
//! solver.train(10_000);
//!
//! // 4. Get strategies
//! let strategy = solver.get_average_strategy("info_key", num_actions);
//! ```
//!
//! ## Modules
//!
//! - [`cfr`]: Core CFR algorithm and solver
//! - [`games`]: Example game implementations (Kuhn Poker, etc.)
//!
//! ## Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────┐
//! │                      CFR Solver (Generic)                       │
//! │  - Regret accumulation    - Strategy computation                │
//! │  - MCCFR traversal        - Exploitability calculation          │
//! └─────────────────────────────────────────────────────────────────┘
//!                               │
//!                               │ implements Game trait
//!                               ▼
//!         ┌─────────────────────┼─────────────────────┐
//!         │                     │                     │
//!         ▼                     ▼                     ▼
//!    ┌─────────┐         ┌───────────┐         ┌───────────┐
//!    │  Kuhn   │         │ Push/Fold │         │  Preflop  │
//!    │  Poker  │         │  Poker    │         │  Solver   │
//!    └─────────┘         └───────────┘         └───────────┘
//! ```

#![warn(missing_docs)]

/// CFR (Counterfactual Regret Minimization) solver module.
///
/// This is the core module containing the generic CFR algorithm.
pub mod cfr;

/// Game implementations module.
///
/// Contains example games like Kuhn Poker for testing and validation.
pub mod games;

// Re-export commonly used types at crate root for convenience
pub use cfr::{Action, CFRConfig, CFRSolver, CFRStats, Game, GameState, InfoState};
