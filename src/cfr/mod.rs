//! CFR (Counterfactual Regret Minimization) Solver Module.
//!
//! This module provides a generic implementation of the CFR algorithm family
//! for computing Nash equilibrium strategies in extensive-form games.
//!
//! # Overview
//!
//! CFR is an iterative algorithm that converges to Nash equilibrium by:
//! 1. Computing counterfactual regret for each action at each decision point
//! 2. Updating strategies to minimize regret over time
//! 3. Averaging strategies across iterations to converge to equilibrium
//!
//! # Supported Variants
//!
//! - **Vanilla CFR**: Original algorithm with full tree traversal
//! - **CFR+**: Floors negative regrets to zero for faster convergence
//! - **Linear CFR**: Weights later iterations more heavily
//! - **MCCFR (Monte Carlo CFR)**: Samples the game tree for scalability
//! - **Discounted CFR**: Discounts older regrets/strategies
//!
//! # Usage
//!
//! To use the CFR solver with your game:
//!
//! 1. Implement the `Game` trait for your game
//! 2. Create a `CFRSolver` with your game and configuration
//! 3. Call `train()` to run iterations
//! 4. Extract strategies using `get_average_strategy()`
//!
//! # Example
//!
//! ```ignore
//! use rust_solver_poc::cfr::{Game, CFRSolver, CFRConfig};
//!
//! // Define your game by implementing the Game trait
//! struct MyGame { /* ... */ }
//! impl Game for MyGame { /* ... */ }
//!
//! // Create and train the solver
//! let game = MyGame::new();
//! let config = CFRConfig::default();
//! let mut solver = CFRSolver::new(game, config);
//!
//! // Train for 10,000 iterations
//! let stats = solver.train(10_000);
//! println!("Trained {} info sets in {:.2}s", stats.info_sets, stats.elapsed_seconds);
//!
//! // Get equilibrium strategy for a specific information set
//! let strategy = solver.get_average_strategy("my_info_key", 3);
//! println!("Strategy: {:?}", strategy);
//! ```
//!
//! # Theory
//!
//! CFR is based on the principle of regret minimization:
//!
//! **Regret**: The difference between the value of an action and the value of the current strategy.
//! ```text
//! Regret(a) = Value(a) - Value(current_strategy)
//! ```
//!
//! **Regret Matching**: Set strategy proportional to positive regrets.
//! ```text
//! Strategy(a) = max(0, Regret(a)) / sum(max(0, Regret(a')))
//! ```
//!
//! **Convergence**: Average regret decreases as O(1/sqrt(T)), and the average strategy
//! converges to Nash equilibrium.
//!
//! # References
//!
//! - Zinkevich, M., et al. "Regret Minimization in Games with Incomplete Information" (2007)
//! - Tammelin, O. "Solving Large Imperfect Information Games Using CFR+" (2014)
//! - Brown, N., Sandholm, T. "Solving Imperfect-Information Games via Discounted Regret Minimization" (2019)

pub mod config;
pub mod game;
pub mod solver;
pub mod storage;

// Re-export main types for convenient access
pub use config::{CFRConfig, CFRStats, ConfigError, ExploitabilityPoint};
pub use game::{Action, Game, GameState, InfoState};
pub use solver::{CFRSolver, SolverState};
pub use storage::{RegretStorage, StorageExport};
