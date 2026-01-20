//! Configuration options for the CFR solver.
//!
//! This module provides configuration structs that control the behavior
//! of the CFR algorithm, including variants like CFR+ and Linear CFR.

use serde::{Deserialize, Serialize};

/// Configuration for the CFR solver.
///
/// This struct controls various aspects of the CFR algorithm including:
/// - Which CFR variant to use (vanilla, CFR+, Linear CFR)
/// - Exploration parameters for MCCFR
/// - Convergence thresholds
///
/// # Example
/// ```
/// use rust_solver_poc::cfr::CFRConfig;
///
/// let config = CFRConfig::default();
/// assert!(config.use_cfr_plus); // CFR+ is enabled by default
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CFRConfig {
    /// Use CFR+ variant (reset negative regrets to 0).
    ///
    /// CFR+ typically converges faster than vanilla CFR by preventing
    /// negative regrets from accumulating. This is enabled by default.
    pub use_cfr_plus: bool,

    /// Use Linear CFR weighting (weight iterations linearly).
    ///
    /// Linear CFR gives more weight to later iterations when computing
    /// the average strategy. This often improves convergence speed.
    pub use_linear_cfr: bool,

    /// Exploration probability for Monte Carlo sampling.
    ///
    /// When sampling opponent actions in MCCFR, this is the probability
    /// of choosing a random action instead of sampling from the current
    /// strategy. Higher values explore more but may slow convergence.
    ///
    /// Recommended range: 0.3 - 0.6
    pub exploration: f64,

    /// Minimum regret floor (for vanilla CFR without CFR+).
    ///
    /// If `use_cfr_plus` is false, regrets are floored at this value
    /// to prevent extreme negative regrets. Set to `f64::NEG_INFINITY`
    /// for true vanilla CFR.
    pub regret_floor: f64,

    /// Discount factor for regrets (for Discounted CFR).
    ///
    /// If set, older regrets are discounted by this factor each iteration.
    /// A value of 1.0 means no discounting (standard CFR/CFR+).
    /// A value of 0.0 would forget all history (not recommended).
    ///
    /// Set to `None` to disable discounting.
    pub regret_discount: Option<f64>,

    /// Discount factor for strategy sums (for Discounted CFR).
    ///
    /// Similar to regret_discount but for the cumulative strategy.
    /// Set to `None` to disable discounting.
    pub strategy_discount: Option<f64>,

    /// Number of threads to use for parallel MCCFR.
    ///
    /// Set to 0 or 1 for single-threaded execution.
    /// Set to `None` to use all available cores.
    pub num_threads: Option<usize>,

    /// Random seed for reproducibility.
    ///
    /// If set, the solver will use this seed for random number generation,
    /// making results reproducible. If `None`, a random seed is used.
    pub seed: Option<u64>,
}

impl Default for CFRConfig {
    fn default() -> Self {
        Self {
            use_cfr_plus: true,
            use_linear_cfr: true,
            exploration: 0.0,  // Standard external sampling (no exploration)
            regret_floor: f64::NEG_INFINITY,
            regret_discount: None,
            strategy_discount: None,
            num_threads: None,
            seed: None,
        }
    }
}

impl CFRConfig {
    /// Create a new CFRConfig with default settings.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a configuration optimized for fast convergence.
    ///
    /// This uses CFR+ with linear weighting and moderate exploration.
    pub fn fast() -> Self {
        Self {
            use_cfr_plus: true,
            use_linear_cfr: true,
            exploration: 0.4,
            ..Default::default()
        }
    }

    /// Create a configuration for vanilla CFR (for comparison/testing).
    ///
    /// This disables all enhancements for a pure CFR implementation.
    pub fn vanilla() -> Self {
        Self {
            use_cfr_plus: false,
            use_linear_cfr: false,
            exploration: 0.6,
            regret_floor: f64::NEG_INFINITY,
            regret_discount: None,
            strategy_discount: None,
            ..Default::default()
        }
    }

    /// Create a configuration with discounted CFR.
    ///
    /// Discounted CFR can help with games that have high variance
    /// or when you want to weight recent iterations more heavily.
    ///
    /// # Arguments
    /// * `alpha` - Regret discount factor (typically 0.75 - 0.99)
    /// * `beta` - Strategy discount factor (typically 0.0 - 0.5)
    pub fn discounted(alpha: f64, beta: f64) -> Self {
        Self {
            use_cfr_plus: true,
            use_linear_cfr: false, // Usually disabled with discounting
            exploration: 0.6,
            regret_discount: Some(alpha),
            strategy_discount: Some(beta),
            ..Default::default()
        }
    }

    /// Builder method: set whether to use CFR+.
    pub fn with_cfr_plus(mut self, enable: bool) -> Self {
        self.use_cfr_plus = enable;
        self
    }

    /// Builder method: set whether to use Linear CFR.
    pub fn with_linear_cfr(mut self, enable: bool) -> Self {
        self.use_linear_cfr = enable;
        self
    }

    /// Builder method: set exploration probability.
    pub fn with_exploration(mut self, exploration: f64) -> Self {
        self.exploration = exploration.clamp(0.0, 1.0);
        self
    }

    /// Builder method: set number of threads.
    pub fn with_threads(mut self, threads: usize) -> Self {
        self.num_threads = Some(threads);
        self
    }

    /// Builder method: set random seed.
    pub fn with_seed(mut self, seed: u64) -> Self {
        self.seed = Some(seed);
        self
    }

    /// Validate the configuration and return any errors.
    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.exploration < 0.0 || self.exploration > 1.0 {
            return Err(ConfigError::InvalidExploration(self.exploration));
        }

        if let Some(discount) = self.regret_discount {
            if discount < 0.0 || discount > 1.0 {
                return Err(ConfigError::InvalidDiscount("regret", discount));
            }
        }

        if let Some(discount) = self.strategy_discount {
            if discount < 0.0 || discount > 1.0 {
                return Err(ConfigError::InvalidDiscount("strategy", discount));
            }
        }

        Ok(())
    }
}

/// Errors that can occur when validating CFR configuration.
#[derive(Debug, Clone)]
pub enum ConfigError {
    /// Exploration probability is out of range [0, 1].
    InvalidExploration(f64),
    /// Discount factor is out of range [0, 1].
    InvalidDiscount(&'static str, f64),
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConfigError::InvalidExploration(val) => {
                write!(f, "Exploration probability {} is out of range [0, 1]", val)
            }
            ConfigError::InvalidDiscount(name, val) => {
                write!(f, "{} discount {} is out of range [0, 1]", name, val)
            }
        }
    }
}

impl std::error::Error for ConfigError {}

/// Statistics tracked during CFR training.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CFRStats {
    /// Total number of iterations completed.
    pub iterations: u64,

    /// Number of unique information sets discovered.
    pub info_sets: usize,

    /// Total time spent training (in seconds).
    pub elapsed_seconds: f64,

    /// Iterations per second.
    pub iterations_per_second: f64,

    /// Estimated exploitability (if calculated).
    pub exploitability: Option<f64>,

    /// History of exploitability measurements.
    pub exploitability_history: Vec<ExploitabilityPoint>,
}

/// A single exploitability measurement at a specific iteration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExploitabilityPoint {
    /// Iteration number when this measurement was taken.
    pub iteration: u64,
    /// Exploitability value (in milli-big-blinds per hand for poker).
    pub exploitability: f64,
}

impl CFRStats {
    /// Create new empty stats.
    pub fn new() -> Self {
        Self::default()
    }

    /// Update iterations per second based on elapsed time.
    pub fn update_rate(&mut self) {
        if self.elapsed_seconds > 0.0 {
            self.iterations_per_second = self.iterations as f64 / self.elapsed_seconds;
        }
    }

    /// Record an exploitability measurement.
    pub fn record_exploitability(&mut self, iteration: u64, exploitability: f64) {
        self.exploitability = Some(exploitability);
        self.exploitability_history.push(ExploitabilityPoint {
            iteration,
            exploitability,
        });
    }
}
