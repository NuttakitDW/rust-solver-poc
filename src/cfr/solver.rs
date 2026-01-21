//! Monte Carlo Counterfactual Regret Minimization (MCCFR) Solver.
//!
//! This module implements the core CFR algorithm with several variants:
//! - **Vanilla CFR**: Original algorithm with full tree traversal
//! - **CFR+**: Floors negative regrets to zero for faster convergence
//! - **Linear CFR**: Weights later iterations more heavily
//! - **MCCFR**: Monte Carlo sampling for large games
//!
//! The solver is generic over any game that implements the `Game` trait.

use std::marker::PhantomData;
use std::time::Instant;

use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

use crate::cfr::config::{CFRConfig, CFRStats};
use crate::cfr::game::{Game, InfoState};
use crate::cfr::storage::RegretStorage;

/// The main CFR solver.
///
/// This struct manages the CFR algorithm for any game implementing the `Game` trait.
/// It handles regret accumulation, strategy computation, and iterative solving.
///
/// # Type Parameters
/// - `G`: The game type implementing the `Game` trait
///
/// # Example
/// ```ignore
/// use rust_solver_poc::cfr::{CFRSolver, CFRConfig};
///
/// let game = MyGame::new();
/// let config = CFRConfig::default();
/// let mut solver = CFRSolver::new(game, config);
///
/// // Train for 10,000 iterations
/// solver.train(10_000);
///
/// // Get the resulting strategy
/// let strategy = solver.get_average_strategy("info_key", 2);
/// ```
pub struct CFRSolver<G: Game> {
    /// The game being solved.
    game: G,

    /// Configuration for the solver.
    config: CFRConfig,

    /// Storage for regrets and strategy sums.
    storage: RegretStorage,

    /// Current iteration count.
    iteration: u64,

    /// Statistics tracking.
    stats: CFRStats,

    /// Random number generator.
    rng: StdRng,

    /// Phantom data for type safety.
    _phantom: PhantomData<G>,
}

impl<G: Game> CFRSolver<G> {
    /// Create a new CFR solver for the given game.
    ///
    /// # Arguments
    /// * `game` - The game to solve
    /// * `config` - Configuration options for the solver
    pub fn new(game: G, config: CFRConfig) -> Self {
        let rng = match config.seed {
            Some(seed) => StdRng::seed_from_u64(seed),
            None => StdRng::from_entropy(),
        };

        Self {
            game,
            config,
            storage: RegretStorage::new(),
            iteration: 0,
            stats: CFRStats::new(),
            rng,
            _phantom: PhantomData,
        }
    }

    /// Create a solver with pre-allocated storage capacity.
    ///
    /// Use this when you have an estimate of how many info sets the game has
    /// to avoid reallocations during training.
    pub fn with_capacity(game: G, config: CFRConfig, capacity: usize) -> Self {
        let rng = match config.seed {
            Some(seed) => StdRng::seed_from_u64(seed),
            None => StdRng::from_entropy(),
        };

        Self {
            game,
            config,
            storage: RegretStorage::with_capacity(capacity),
            iteration: 0,
            stats: CFRStats::new(),
            rng,
            _phantom: PhantomData,
        }
    }

    /// Run a single iteration of MCCFR.
    ///
    /// This traverses the game tree once for each player, updating regrets
    /// and strategy sums along the way.
    pub fn run_iteration(&mut self) {
        self.iteration += 1;

        // Apply discounting if configured
        if let Some(discount) = self.config.regret_discount {
            self.storage.discount_regrets(discount);
        }
        if let Some(discount) = self.config.strategy_discount {
            self.storage.discount_strategy_sums(discount);
        }

        // Traverse for each player
        for player in 0..self.game.num_players() {
            let initial_state = self.game.initial_state();
            let reach_probs = vec![1.0; self.game.num_players()];

            self.traverse(&initial_state, player, reach_probs);
        }
    }

    /// Train the solver for a specified number of iterations.
    ///
    /// # Arguments
    /// * `iterations` - Number of iterations to run
    ///
    /// # Returns
    /// Statistics from the training run.
    pub fn train(&mut self, iterations: u64) -> &CFRStats {
        let start_time = Instant::now();

        for _ in 0..iterations {
            self.run_iteration();
        }

        // Update stats
        self.stats.iterations = self.iteration;
        self.stats.info_sets = self.storage.num_info_sets();
        self.stats.elapsed_seconds = start_time.elapsed().as_secs_f64();
        self.stats.update_rate();

        &self.stats
    }

    /// Train with a callback for progress tracking.
    ///
    /// # Arguments
    /// * `iterations` - Number of iterations to run
    /// * `callback_interval` - How often to call the callback
    /// * `callback` - Function called every `callback_interval` iterations
    pub fn train_with_callback<F>(
        &mut self,
        iterations: u64,
        callback_interval: u64,
        mut callback: F,
    ) -> &CFRStats
    where
        F: FnMut(&CFRStats),
    {
        let start_time = Instant::now();

        for i in 0..iterations {
            self.run_iteration();

            if (i + 1) % callback_interval == 0 {
                self.stats.iterations = self.iteration;
                self.stats.info_sets = self.storage.num_info_sets();
                self.stats.elapsed_seconds = start_time.elapsed().as_secs_f64();
                self.stats.update_rate();
                callback(&self.stats);
            }
        }

        // Final stats update
        self.stats.iterations = self.iteration;
        self.stats.info_sets = self.storage.num_info_sets();
        self.stats.elapsed_seconds = start_time.elapsed().as_secs_f64();
        self.stats.update_rate();

        &self.stats
    }

    /// Train until the Convergence Indicator (CI) reaches the target value.
    ///
    /// CI measures how much strategies have changed during recent iterations.
    /// Lower CI means better convergence:
    /// - CI < 10: bare minimum for a usable solution
    /// - CI ~ 1: close to fully converged (Nash equilibrium)
    ///
    /// # Arguments
    /// * `ci_target` - Target CI value to reach (e.g., 10.0 for minimum, 1.0 for full)
    /// * `ci_check_interval` - How many iterations between CI checks
    /// * `max_iterations` - Maximum iterations before giving up (0 = no limit)
    /// * `callback` - Optional callback for progress updates
    ///
    /// # Returns
    /// Final CI value achieved
    pub fn train_until_converged<F>(
        &mut self,
        ci_target: f64,
        ci_check_interval: u64,
        max_iterations: u64,
        mut callback: Option<F>,
    ) -> ConvergenceResult
    where
        F: FnMut(&ConvergenceStats),
    {
        use crate::cfr::storage::StrategySnapshot;

        let start_time = Instant::now();
        let mut snapshot: Option<StrategySnapshot> = None;
        let mut current_ci = f64::INFINITY;

        // Minimum iterations before first CI check (need enough data to be meaningful)
        // CI can be misleadingly low early on when info sets haven't been visited enough
        let warmup_iterations = ci_check_interval.max(1000);

        loop {
            // Run a batch of iterations
            for _ in 0..ci_check_interval {
                self.run_iteration();
            }

            let elapsed = start_time.elapsed().as_secs_f64();
            let iters_per_sec = if elapsed > 0.0 {
                self.iteration as f64 / elapsed
            } else {
                0.0
            };

            // Check convergence after warmup
            if self.iteration >= warmup_iterations {
                // Take snapshot if we don't have one
                if snapshot.is_none() {
                    snapshot = Some(self.storage.snapshot_strategies());
                    // Still report progress (CI will show as infinity/warming)
                    let conv_stats = ConvergenceStats {
                        iteration: self.iteration,
                        ci: current_ci,
                        info_sets: self.storage.num_info_sets(),
                        elapsed_seconds: elapsed,
                        iterations_per_second: iters_per_sec,
                    };
                    if let Some(ref mut cb) = callback {
                        cb(&conv_stats);
                    }
                    continue;
                }

                // Calculate CI
                current_ci = self.storage.calculate_ci(snapshot.as_ref().unwrap());

                // Update stats and callback
                let conv_stats = ConvergenceStats {
                    iteration: self.iteration,
                    ci: current_ci,
                    info_sets: self.storage.num_info_sets(),
                    elapsed_seconds: elapsed,
                    iterations_per_second: iters_per_sec,
                };

                if let Some(ref mut cb) = callback {
                    cb(&conv_stats);
                }

                // Check if converged (require minimum iterations to avoid false convergence)
                // CI can be misleadingly low early when strategies haven't been visited enough
                let min_iterations_for_convergence = 5000u64;
                if current_ci <= ci_target && self.iteration >= min_iterations_for_convergence {
                    return ConvergenceResult {
                        converged: true,
                        final_ci: current_ci,
                        iterations: self.iteration,
                        elapsed_seconds: elapsed,
                    };
                }

                // Take new snapshot for next CI measurement
                snapshot = Some(self.storage.snapshot_strategies());
            } else {
                // During warmup, still report progress
                let conv_stats = ConvergenceStats {
                    iteration: self.iteration,
                    ci: current_ci, // Will be infinity during warmup
                    info_sets: self.storage.num_info_sets(),
                    elapsed_seconds: elapsed,
                    iterations_per_second: iters_per_sec,
                };
                if let Some(ref mut cb) = callback {
                    cb(&conv_stats);
                }
            }

            // Check max iterations
            if max_iterations > 0 && self.iteration >= max_iterations {
                return ConvergenceResult {
                    converged: false,
                    final_ci: current_ci,
                    iterations: self.iteration,
                    elapsed_seconds: start_time.elapsed().as_secs_f64(),
                };
            }
        }
    }

    /// Get current CI (Convergence Indicator) compared to a snapshot.
    ///
    /// Use `snapshot_strategies()` to take a snapshot, then call this after
    /// more iterations to measure convergence.
    pub fn calculate_ci(&self, snapshot: &crate::cfr::storage::StrategySnapshot) -> f64 {
        self.storage.calculate_ci(snapshot)
    }

    /// Take a snapshot of current average strategies for CI calculation.
    pub fn snapshot_strategies(&self) -> crate::cfr::storage::StrategySnapshot {
        self.storage.snapshot_strategies()
    }

    /// Core MCCFR traversal function.
    ///
    /// This recursively traverses the game tree, computing counterfactual values
    /// and updating regrets. Uses external sampling for opponent actions.
    fn traverse(&mut self, state: &G::State, traverser: usize, reach_probs: Vec<f64>) -> f64 {
        // Terminal node: return payoff
        if self.game.is_terminal(state) {
            return self.game.get_payoff(state, traverser);
        }

        // Chance node: sample outcome and continue
        if self.game.is_chance(state) {
            let new_state = self.game.sample_chance(state, &mut self.rng);
            return self.traverse(&new_state, traverser, reach_probs);
        }

        // Get current player and available actions
        let current_player = match self.game.current_player(state) {
            Some(p) => p,
            None => return self.game.get_payoff(state, traverser),
        };

        let actions = self.game.available_actions(state);
        let num_actions = actions.len();

        if num_actions == 0 {
            return self.game.get_payoff(state, traverser);
        }

        // Get information state and current strategy
        let info_state = self.game.info_state(state);
        let info_key = info_state.key();
        let strategy = self.storage.get_current_strategy(&info_key, num_actions);

        if current_player == traverser {
            // Traverser: explore all actions, update regrets
            self.traverse_player(state, traverser, &reach_probs, &actions, &strategy, &info_key)
        } else {
            // Opponent: sample one action according to strategy
            self.traverse_opponent(state, traverser, reach_probs, &actions, &strategy, current_player)
        }
    }

    /// Handle traversal when it's the traversing player's turn.
    ///
    /// Explores all actions and updates regrets based on counterfactual values.
    fn traverse_player(
        &mut self,
        state: &G::State,
        traverser: usize,
        reach_probs: &[f64],
        actions: &[G::Action],
        strategy: &[f64],
        info_key: &str,
    ) -> f64 {
        let num_actions = actions.len();
        let mut action_values = vec![0.0; num_actions];

        // Explore all actions
        for (i, action) in actions.iter().enumerate() {
            let new_state = self.game.apply_action(state, action);

            // Update reach probabilities
            let mut new_reach = reach_probs.to_vec();
            new_reach[traverser] *= strategy[i];

            action_values[i] = self.traverse(&new_state, traverser, new_reach);
        }

        // Compute node value (expected value over strategy)
        let node_value: f64 = strategy
            .iter()
            .zip(action_values.iter())
            .map(|(&s, &v)| s * v)
            .sum();

        // Compute regret updates: regret[a] = value[a] - node_value
        let regret_updates: Vec<f64> = action_values.iter().map(|&v| v - node_value).collect();

        // Update regrets in storage
        self.storage
            .update_regrets(info_key, &regret_updates, self.config.use_cfr_plus);

        // Store action names (only stored once per info set)
        let action_names: Vec<String> = actions.iter()
            .map(|a| self.game.action_name(a))
            .collect();
        self.storage.set_action_names(info_key, action_names);

        // Update strategy sum for average strategy computation
        let weight = if self.config.use_linear_cfr {
            reach_probs[traverser] * self.iteration as f64
        } else {
            reach_probs[traverser]
        };
        self.storage.update_strategy_sum(info_key, strategy, weight);

        node_value
    }

    /// Handle traversal when it's an opponent's turn.
    ///
    /// Samples one action using external sampling with exploration.
    fn traverse_opponent(
        &mut self,
        state: &G::State,
        traverser: usize,
        mut reach_probs: Vec<f64>,
        actions: &[G::Action],
        strategy: &[f64],
        current_player: usize,
    ) -> f64 {
        // External sampling with exploration
        let action_idx = if self.rng.gen::<f64>() < self.config.exploration {
            // Explore: choose random action
            self.rng.gen_range(0..actions.len())
        } else {
            // Exploit: sample from strategy
            self.sample_action(strategy)
        };

        let action = &actions[action_idx];
        let new_state = self.game.apply_action(state, action);

        // Update reach probability for opponent
        reach_probs[current_player] *= strategy[action_idx];

        self.traverse(&new_state, traverser, reach_probs)
    }

    /// Sample an action index according to a probability distribution.
    fn sample_action(&mut self, strategy: &[f64]) -> usize {
        let r: f64 = self.rng.gen();
        let mut cumsum = 0.0;

        for (i, &prob) in strategy.iter().enumerate() {
            cumsum += prob;
            if r < cumsum {
                return i;
            }
        }

        // Fallback to last action (handles floating point imprecision)
        strategy.len() - 1
    }

    /// Get the current strategy for an information set.
    ///
    /// This returns the strategy based on current regrets (regret matching).
    pub fn get_current_strategy(&self, info_key: &str, num_actions: usize) -> Vec<f64> {
        self.storage.get_current_strategy(info_key, num_actions)
    }

    /// Get the average strategy for an information set.
    ///
    /// This returns the time-averaged strategy which converges to Nash equilibrium.
    pub fn get_average_strategy(&self, info_key: &str, num_actions: usize) -> Vec<f64> {
        self.storage.get_average_strategy(info_key, num_actions)
    }

    /// Get the current iteration count.
    pub fn iteration(&self) -> u64 {
        self.iteration
    }

    /// Get the number of information sets discovered.
    pub fn num_info_sets(&self) -> usize {
        self.storage.num_info_sets()
    }

    /// Get current statistics.
    pub fn stats(&self) -> &CFRStats {
        &self.stats
    }

    /// Get reference to the storage for analysis.
    pub fn storage(&self) -> &RegretStorage {
        &self.storage
    }

    /// Get reference to the game.
    pub fn game(&self) -> &G {
        &self.game
    }

    /// Get reference to the configuration.
    pub fn config(&self) -> &CFRConfig {
        &self.config
    }

    /// Get all information set keys discovered during training.
    pub fn info_set_keys(&self) -> Vec<String> {
        self.storage.regrets().keys().cloned().collect()
    }

    /// Get action names for an information set.
    pub fn get_action_names(&self, info_key: &str) -> Option<Vec<String>> {
        self.storage.get_action_names(info_key)
    }

    /// Calculate exploitability of current strategy.
    ///
    /// Exploitability measures how much value an optimal opponent could gain
    /// against the current strategy. Lower is better; 0 means Nash equilibrium.
    ///
    /// # Arguments
    /// * `num_samples` - Number of samples for Monte Carlo estimation
    ///
    /// # Returns
    /// Estimated exploitability (value the best response gains over current strategy)
    pub fn calculate_exploitability(&mut self, num_samples: usize) -> f64 {
        let mut total_exploitability = 0.0;

        for _ in 0..num_samples {
            for exploiter in 0..self.game.num_players() {
                let initial_state = self.game.initial_state();

                // Value when exploiter plays best response
                let br_value = self.best_response_value(&initial_state, exploiter);

                // Value when exploiter plays current strategy
                let strategy_value = self.strategy_value(&initial_state, exploiter);

                total_exploitability += br_value - strategy_value;
            }
        }

        total_exploitability / (num_samples as f64 * self.game.num_players() as f64)
    }

    /// Compute value when a player plays best response against fixed opponents.
    fn best_response_value(&mut self, state: &G::State, exploiter: usize) -> f64 {
        if self.game.is_terminal(state) {
            return self.game.get_payoff(state, exploiter);
        }

        if self.game.is_chance(state) {
            let new_state = self.game.sample_chance(state, &mut self.rng);
            return self.best_response_value(&new_state, exploiter);
        }

        let current_player = match self.game.current_player(state) {
            Some(p) => p,
            None => return self.game.get_payoff(state, exploiter),
        };

        let actions = self.game.available_actions(state);
        if actions.is_empty() {
            return self.game.get_payoff(state, exploiter);
        }

        if current_player == exploiter {
            // Exploiter: choose best action
            let mut best_value = f64::NEG_INFINITY;
            for action in &actions {
                let new_state = self.game.apply_action(state, action);
                let value = self.best_response_value(&new_state, exploiter);
                best_value = best_value.max(value);
            }
            best_value
        } else {
            // Opponent: play according to average strategy
            let info_state = self.game.info_state(state);
            let strategy = self.storage.get_average_strategy(&info_state.key(), actions.len());

            let mut expected_value = 0.0;
            for (i, action) in actions.iter().enumerate() {
                let new_state = self.game.apply_action(state, action);
                let value = self.best_response_value(&new_state, exploiter);
                expected_value += strategy[i] * value;
            }
            expected_value
        }
    }

    /// Compute value when all players play according to current strategy.
    fn strategy_value(&mut self, state: &G::State, player: usize) -> f64 {
        if self.game.is_terminal(state) {
            return self.game.get_payoff(state, player);
        }

        if self.game.is_chance(state) {
            let new_state = self.game.sample_chance(state, &mut self.rng);
            return self.strategy_value(&new_state, player);
        }

        let _current_player = match self.game.current_player(state) {
            Some(p) => p,
            None => return self.game.get_payoff(state, player),
        };

        let actions = self.game.available_actions(state);
        if actions.is_empty() {
            return self.game.get_payoff(state, player);
        }

        let info_state = self.game.info_state(state);
        let strategy = self.storage.get_average_strategy(&info_state.key(), actions.len());

        let mut expected_value = 0.0;
        for (i, action) in actions.iter().enumerate() {
            let new_state = self.game.apply_action(state, action);
            let value = self.strategy_value(&new_state, player);
            expected_value += strategy[i] * value;
        }
        expected_value
    }

    /// Export solver state for checkpointing.
    pub fn export_state(&self) -> SolverState {
        SolverState {
            iteration: self.iteration,
            storage: self.storage.export(),
            stats: self.stats.clone(),
        }
    }

    /// Import solver state from checkpoint.
    pub fn import_state(&mut self, state: SolverState) {
        self.iteration = state.iteration;
        self.storage.import(state.storage);
        self.stats = state.stats;
    }

    /// Reset the solver to initial state.
    pub fn reset(&mut self) {
        self.storage.clear();
        self.iteration = 0;
        self.stats = CFRStats::new();
    }
}

/// Serializable solver state for checkpointing.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SolverState {
    /// Current iteration.
    pub iteration: u64,
    /// Storage export.
    pub storage: crate::cfr::storage::StorageExport,
    /// Statistics.
    pub stats: CFRStats,
}

impl<G: Game> Clone for CFRSolver<G> {
    fn clone(&self) -> Self {
        Self {
            game: self.game.clone(),
            config: self.config.clone(),
            storage: self.storage.clone(),
            iteration: self.iteration,
            stats: self.stats.clone(),
            rng: StdRng::from_entropy(), // Fresh RNG for clone
            _phantom: PhantomData,
        }
    }
}

/// Statistics during convergence-based training.
#[derive(Debug, Clone)]
pub struct ConvergenceStats {
    /// Current iteration count.
    pub iteration: u64,
    /// Current Convergence Indicator value.
    pub ci: f64,
    /// Number of information sets discovered.
    pub info_sets: usize,
    /// Elapsed time in seconds.
    pub elapsed_seconds: f64,
    /// Current solve speed.
    pub iterations_per_second: f64,
}

/// Result of convergence-based training.
#[derive(Debug, Clone)]
pub struct ConvergenceResult {
    /// Whether the target CI was reached.
    pub converged: bool,
    /// Final CI value achieved.
    pub final_ci: f64,
    /// Total iterations run.
    pub iterations: u64,
    /// Total elapsed time in seconds.
    pub elapsed_seconds: f64,
}
