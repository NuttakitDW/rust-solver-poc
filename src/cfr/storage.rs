//! Storage for CFR regrets and strategies.
//!
//! This module provides thread-safe storage for cumulative regrets and
//! strategy sums used in CFR algorithms.

use rustc_hash::FxHashMap;
use serde::{Deserialize, Serialize};
use std::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};

/// Thread-safe storage for regrets and strategy sums.
///
/// This struct manages the core data structures used by CFR:
/// - **Regrets**: Cumulative counterfactual regret for each action at each info set
/// - **Strategy sums**: Cumulative strategy weights for computing average strategy
///
/// The storage uses interior mutability with `RwLock` to allow concurrent
/// reads and exclusive writes, which is important for parallel CFR.
#[derive(Debug)]
pub struct RegretStorage {
    /// Cumulative regrets: info_key -> [regret per action]
    regrets: RwLock<FxHashMap<String, Vec<f64>>>,

    /// Cumulative strategy sums: info_key -> [strategy weight per action]
    strategy_sums: RwLock<FxHashMap<String, Vec<f64>>>,

    /// Action counts for each info set (to verify consistency)
    action_counts: RwLock<FxHashMap<String, usize>>,

    /// Action names for each info set: info_key -> [action name per action]
    action_names: RwLock<FxHashMap<String, Vec<String>>>,
}

impl Default for RegretStorage {
    fn default() -> Self {
        Self::new()
    }
}

impl RegretStorage {
    /// Create new empty storage.
    pub fn new() -> Self {
        Self {
            regrets: RwLock::new(FxHashMap::default()),
            strategy_sums: RwLock::new(FxHashMap::default()),
            action_counts: RwLock::new(FxHashMap::default()),
            action_names: RwLock::new(FxHashMap::default()),
        }
    }

    /// Create storage with pre-allocated capacity.
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            regrets: RwLock::new(FxHashMap::with_capacity_and_hasher(
                capacity,
                Default::default(),
            )),
            strategy_sums: RwLock::new(FxHashMap::with_capacity_and_hasher(
                capacity,
                Default::default(),
            )),
            action_counts: RwLock::new(FxHashMap::with_capacity_and_hasher(
                capacity,
                Default::default(),
            )),
            action_names: RwLock::new(FxHashMap::with_capacity_and_hasher(
                capacity,
                Default::default(),
            )),
        }
    }

    /// Get current strategy for an info set using regret matching.
    ///
    /// The strategy is proportional to positive regrets. If all regrets are
    /// non-positive, returns a uniform strategy.
    ///
    /// # Arguments
    /// * `info_key` - The information set key
    /// * `num_actions` - Number of available actions
    ///
    /// # Returns
    /// A vector of action probabilities summing to 1.0
    pub fn get_current_strategy(&self, info_key: &str, num_actions: usize) -> Vec<f64> {
        let regrets = self.regrets.read().unwrap();

        match regrets.get(info_key) {
            Some(r) => {
                // Regret matching: strategy proportional to positive regrets
                let positive: Vec<f64> = r.iter().map(|&x| x.max(0.0)).collect();
                let sum: f64 = positive.iter().sum();

                if sum > 0.0 {
                    positive.iter().map(|&x| x / sum).collect()
                } else {
                    // Uniform if no positive regrets
                    vec![1.0 / num_actions as f64; num_actions]
                }
            }
            None => {
                // Uniform for unseen info sets
                vec![1.0 / num_actions as f64; num_actions]
            }
        }
    }

    /// Get average strategy for an info set (Nash equilibrium approximation).
    ///
    /// The average strategy is computed from cumulative strategy sums,
    /// weighted by iteration (if using Linear CFR) or uniform.
    ///
    /// # Arguments
    /// * `info_key` - The information set key
    /// * `num_actions` - Number of available actions
    ///
    /// # Returns
    /// A vector of action probabilities summing to 1.0
    pub fn get_average_strategy(&self, info_key: &str, num_actions: usize) -> Vec<f64> {
        let strategy_sums = self.strategy_sums.read().unwrap();

        match strategy_sums.get(info_key) {
            Some(sums) => {
                let total: f64 = sums.iter().sum();
                if total > 0.0 {
                    sums.iter().map(|&x| x / total).collect()
                } else {
                    vec![1.0 / num_actions as f64; num_actions]
                }
            }
            None => {
                vec![1.0 / num_actions as f64; num_actions]
            }
        }
    }

    /// Update regrets for an info set.
    ///
    /// # Arguments
    /// * `info_key` - The information set key
    /// * `regret_updates` - Regret delta for each action (action_value - node_value)
    /// * `use_cfr_plus` - If true, floor negative regrets to 0
    pub fn update_regrets(&self, info_key: &str, regret_updates: &[f64], use_cfr_plus: bool) {
        let mut regrets = self.regrets.write().unwrap();
        let mut action_counts = self.action_counts.write().unwrap();

        let num_actions = regret_updates.len();

        // Initialize or get existing regrets
        let entry = regrets
            .entry(info_key.to_string())
            .or_insert_with(|| vec![0.0; num_actions]);

        // Verify action count consistency
        if let Some(&stored_count) = action_counts.get(info_key) {
            debug_assert_eq!(
                stored_count, num_actions,
                "Action count mismatch for info set {}",
                info_key
            );
        } else {
            action_counts.insert(info_key.to_string(), num_actions);
        }

        // Update regrets
        for (i, &update) in regret_updates.iter().enumerate() {
            entry[i] += update;

            // CFR+: floor negative regrets to 0
            if use_cfr_plus && entry[i] < 0.0 {
                entry[i] = 0.0;
            }
        }
    }

    /// Update strategy sum for an info set.
    ///
    /// # Arguments
    /// * `info_key` - The information set key
    /// * `strategy` - Current strategy for each action
    /// * `weight` - Weight to apply (typically reach probability * iteration weight)
    pub fn update_strategy_sum(&self, info_key: &str, strategy: &[f64], weight: f64) {
        let mut strategy_sums = self.strategy_sums.write().unwrap();

        let num_actions = strategy.len();

        let entry = strategy_sums
            .entry(info_key.to_string())
            .or_insert_with(|| vec![0.0; num_actions]);

        for (i, &prob) in strategy.iter().enumerate() {
            entry[i] += prob * weight;
        }
    }

    /// Store action names for an info set (only stores if not already present).
    ///
    /// # Arguments
    /// * `info_key` - The information set key
    /// * `names` - Names for each available action
    pub fn set_action_names(&self, info_key: &str, names: Vec<String>) {
        let mut action_names = self.action_names.write().unwrap();
        action_names.entry(info_key.to_string()).or_insert(names);
    }

    /// Get action names for an info set.
    ///
    /// # Arguments
    /// * `info_key` - The information set key
    ///
    /// # Returns
    /// Action names if stored, None otherwise
    pub fn get_action_names(&self, info_key: &str) -> Option<Vec<String>> {
        let action_names = self.action_names.read().unwrap();
        action_names.get(info_key).cloned()
    }

    /// Get read access to action names map.
    pub fn action_names(&self) -> RwLockReadGuard<'_, FxHashMap<String, Vec<String>>> {
        self.action_names.read().unwrap()
    }

    /// Apply discount to all regrets (for Discounted CFR).
    ///
    /// # Arguments
    /// * `discount` - Discount factor (0 to 1)
    pub fn discount_regrets(&self, discount: f64) {
        let mut regrets = self.regrets.write().unwrap();

        for values in regrets.values_mut() {
            for v in values.iter_mut() {
                *v *= discount;
            }
        }
    }

    /// Apply discount to all strategy sums (for Discounted CFR).
    ///
    /// # Arguments
    /// * `discount` - Discount factor (0 to 1)
    pub fn discount_strategy_sums(&self, discount: f64) {
        let mut strategy_sums = self.strategy_sums.write().unwrap();

        for values in strategy_sums.values_mut() {
            for v in values.iter_mut() {
                *v *= discount;
            }
        }
    }

    /// Get the number of information sets stored.
    pub fn num_info_sets(&self) -> usize {
        self.regrets.read().unwrap().len()
    }

    /// Check if an info set exists in storage.
    pub fn contains(&self, info_key: &str) -> bool {
        self.regrets.read().unwrap().contains_key(info_key)
    }

    /// Get read access to regrets (for analysis/export).
    pub fn regrets(&self) -> RwLockReadGuard<'_, FxHashMap<String, Vec<f64>>> {
        self.regrets.read().unwrap()
    }

    /// Get read access to strategy sums (for analysis/export).
    pub fn strategy_sums(&self) -> RwLockReadGuard<'_, FxHashMap<String, Vec<f64>>> {
        self.strategy_sums.read().unwrap()
    }

    /// Get mutable access to regrets (for loading checkpoints).
    pub fn regrets_mut(&self) -> RwLockWriteGuard<'_, FxHashMap<String, Vec<f64>>> {
        self.regrets.write().unwrap()
    }

    /// Get mutable access to strategy sums (for loading checkpoints).
    pub fn strategy_sums_mut(&self) -> RwLockWriteGuard<'_, FxHashMap<String, Vec<f64>>> {
        self.strategy_sums.write().unwrap()
    }

    /// Clear all stored data.
    pub fn clear(&self) {
        self.regrets.write().unwrap().clear();
        self.strategy_sums.write().unwrap().clear();
        self.action_counts.write().unwrap().clear();
        self.action_names.write().unwrap().clear();
    }

    /// Get total memory usage estimate in bytes.
    pub fn memory_usage(&self) -> usize {
        let regrets = self.regrets.read().unwrap();
        let strategy_sums = self.strategy_sums.read().unwrap();

        let regret_size: usize = regrets
            .iter()
            .map(|(k, v)| k.len() + v.len() * std::mem::size_of::<f64>())
            .sum();

        let strategy_size: usize = strategy_sums
            .iter()
            .map(|(k, v)| k.len() + v.len() * std::mem::size_of::<f64>())
            .sum();

        regret_size + strategy_size
    }

    /// Export storage to serializable format.
    pub fn export(&self) -> StorageExport {
        StorageExport {
            regrets: self.regrets.read().unwrap().clone(),
            strategy_sums: self.strategy_sums.read().unwrap().clone(),
            action_names: self.action_names.read().unwrap().clone(),
        }
    }

    /// Import storage from serialized format.
    pub fn import(&self, data: StorageExport) {
        *self.regrets.write().unwrap() = data.regrets;
        *self.strategy_sums.write().unwrap() = data.strategy_sums;
        *self.action_names.write().unwrap() = data.action_names;

        // Rebuild action counts
        let mut action_counts = self.action_counts.write().unwrap();
        action_counts.clear();
        for (key, values) in self.regrets.read().unwrap().iter() {
            action_counts.insert(key.clone(), values.len());
        }
    }
}

/// Serializable export format for storage.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageExport {
    /// Cumulative regrets
    pub regrets: FxHashMap<String, Vec<f64>>,
    /// Cumulative strategy sums
    pub strategy_sums: FxHashMap<String, Vec<f64>>,
    /// Action names for each info set
    #[serde(default)]
    pub action_names: FxHashMap<String, Vec<String>>,
}

/// Snapshot of average strategies for CI calculation.
#[derive(Debug, Clone, Default)]
pub struct StrategySnapshot {
    /// Average strategies: info_key -> [probability per action]
    pub strategies: FxHashMap<String, Vec<f64>>,
    /// Strategy sum totals: info_key -> sum of all strategy weights
    /// Used to determine if an info set has been visited
    pub totals: FxHashMap<String, f64>,
}

impl RegretStorage {
    /// Create a snapshot of all current average strategies.
    ///
    /// Used for calculating Convergence Indicator (CI).
    pub fn snapshot_strategies(&self) -> StrategySnapshot {
        let strategy_sums = self.strategy_sums.read().unwrap();
        let action_counts = self.action_counts.read().unwrap();

        let mut strategies = FxHashMap::default();
        let mut totals = FxHashMap::default();

        for (key, sums) in strategy_sums.iter() {
            let num_actions = action_counts.get(key).copied().unwrap_or(sums.len());
            let total: f64 = sums.iter().sum();

            let avg_strategy = if total > 0.0 {
                sums.iter().map(|&x| x / total).collect()
            } else {
                vec![1.0 / num_actions as f64; num_actions]
            };

            strategies.insert(key.clone(), avg_strategy);
            totals.insert(key.clone(), total);
        }

        StrategySnapshot { strategies, totals }
    }

    /// Calculate Convergence Indicator (CI) by comparing current strategies to a snapshot.
    ///
    /// CI measures how much strategies have changed since the snapshot.
    /// Higher CI = strategies changing rapidly (early training, far from Nash)
    /// Lower CI = strategies stabilizing (converging toward Nash equilibrium)
    ///
    /// - CI > 20: very early training, strategies unstable
    /// - CI 10-20: still learning
    /// - CI < 10: bare minimum for usable solution
    /// - CI ~ 1: fully converged
    ///
    /// Formula: CI = 100 * average(sum of |new_prob - old_prob| for each action)
    /// Only counts info sets that have been visited (total > 0) in at least one snapshot.
    ///
    /// # Arguments
    /// * `snapshot` - Previous strategy snapshot to compare against
    ///
    /// # Returns
    /// The CI value (lower is better)
    pub fn calculate_ci(&self, snapshot: &StrategySnapshot) -> f64 {
        let strategy_sums = self.strategy_sums.read().unwrap();
        let action_counts = self.action_counts.read().unwrap();

        let mut total_change = 0.0;
        let mut num_info_sets = 0;

        for (key, sums) in strategy_sums.iter() {
            let num_actions = action_counts.get(key).copied().unwrap_or(sums.len());
            let current_total: f64 = sums.iter().sum();

            // Get old total from snapshot (0 if not present)
            let old_total = snapshot.totals.get(key).copied().unwrap_or(0.0);

            // Only count this info set if it was visited in at least one snapshot
            // This avoids counting uniform vs uniform comparisons (both unvisited)
            if current_total == 0.0 && old_total == 0.0 {
                continue;
            }

            let new_strategy: Vec<f64> = if current_total > 0.0 {
                sums.iter().map(|&x| x / current_total).collect()
            } else {
                vec![1.0 / num_actions as f64; num_actions]
            };

            // Compare with snapshot
            if let Some(old_strategy) = snapshot.strategies.get(key) {
                let change: f64 = new_strategy
                    .iter()
                    .zip(old_strategy.iter())
                    .map(|(&new, &old)| (new - old).abs())
                    .sum();

                total_change += change;
                num_info_sets += 1;
            } else {
                // New info set discovered since snapshot - count as max change from uniform
                // This ensures new info sets contribute to CI
                let uniform_prob = 1.0 / num_actions as f64;
                let change: f64 = new_strategy
                    .iter()
                    .map(|&prob| (prob - uniform_prob).abs())
                    .sum();
                total_change += change;
                num_info_sets += 1;
            }
        }

        if num_info_sets == 0 {
            return f64::INFINITY; // No comparison possible
        }

        // CI = 100 * average change per info set
        // Each info set contributes sum of |delta| which is at most 2.0
        // So we scale by 100 to get a nicer range (max CI = 200)
        100.0 * total_change / num_info_sets as f64
    }

    /// Calculate exploitability-based CI using accumulated regrets.
    ///
    /// This measures actual solution quality, not just stability.
    /// Higher CI = more exploitable (worse solution)
    /// Lower CI = less exploitable (better solution)
    ///
    /// Formula: CI = (sum of positive regrets) / (iterations * num_info_sets) * scaling
    ///
    /// This starts HIGH and decreases as the solver learns.
    /// - CI > 20: very exploitable, needs more training
    /// - CI 10-20: somewhat exploitable
    /// - CI < 10: acceptable solution quality
    /// - CI ~ 1: near Nash equilibrium
    ///
    /// # Arguments
    /// * `iterations` - Number of training iterations completed
    ///
    /// # Returns
    /// The CI value (lower is better)
    pub fn calculate_exploitability_ci(&self, iterations: u64) -> f64 {
        if iterations == 0 {
            return f64::INFINITY;
        }

        let regrets = self.regrets.read().unwrap();

        if regrets.is_empty() {
            return f64::INFINITY;
        }

        let mut total_positive_regret = 0.0;
        let mut num_info_sets = 0;

        for (_key, regret_vec) in regrets.iter() {
            // Sum of positive regrets for this info set
            // In CFR, exploitability is bounded by average positive regret
            let positive_regret: f64 = regret_vec.iter().map(|&r| r.max(0.0)).sum();
            total_positive_regret += positive_regret;
            num_info_sets += 1;
        }

        if num_info_sets == 0 {
            return f64::INFINITY;
        }

        // Exploitability bound: O(R / T) where R is sum of regrets, T is iterations
        // We normalize by info sets to get average regret per info set per iteration
        // Then scale to match typical CI ranges (10 = minimum, 1 = converged)
        //
        // The scaling factor of 1000 is chosen empirically to give similar ranges to HRC:
        // - Early training: CI ~ 20-50
        // - After some training: CI ~ 10-20
        // - Converged: CI < 10
        let avg_regret_per_infoset = total_positive_regret / num_info_sets as f64;
        let normalized_regret = avg_regret_per_infoset / iterations as f64;

        // Scale to get CI in a reasonable range
        // Factor of 1000 gives roughly: initial CI ~ 50, converged CI ~ 1
        1000.0 * normalized_regret
    }
}

impl Clone for RegretStorage {
    fn clone(&self) -> Self {
        Self {
            regrets: RwLock::new(self.regrets.read().unwrap().clone()),
            strategy_sums: RwLock::new(self.strategy_sums.read().unwrap().clone()),
            action_counts: RwLock::new(self.action_counts.read().unwrap().clone()),
            action_names: RwLock::new(self.action_names.read().unwrap().clone()),
        }
    }
}
