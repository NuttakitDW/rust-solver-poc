//! Postflop configuration for the full game solver.
//!
//! This module provides configuration structures for postflop betting trees
//! that can be loaded from JSON files, compatible with HRC-style settings.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

/// Complete configuration for the SB vs BB full game solver.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FullGameConfig {
    /// Version string
    pub version: String,
    /// Configuration name
    pub name: String,
    /// Description
    #[serde(default)]
    pub description: String,
    /// Stack depth in BB
    pub stack_bb: f64,
    /// Blind structure
    pub blinds: BlindsConfig,
    /// Preflop betting configuration
    pub preflop: PreflopBettingConfig,
    /// Postflop betting configuration
    pub postflop: PostflopBettingConfig,
    /// Card abstraction settings
    pub abstraction: AbstractionSettings,
    /// Solver settings
    #[serde(default)]
    pub solver: SolverSettings,
}

/// Blind structure configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlindsConfig {
    pub bb: f64,
    pub sb: f64,
    #[serde(default)]
    pub ante: f64,
}

impl Default for BlindsConfig {
    fn default() -> Self {
        Self {
            bb: 1.0,
            sb: 0.5,
            ante: 0.0,
        }
    }
}

/// Preflop betting configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreflopBettingConfig {
    /// SB open raise size (in BB)
    pub sb_open: f64,
    /// BB 3bet multiplier (of open raise)
    pub bb_3bet_multiplier: f64,
    /// SB 4bet multiplier (of 3bet)
    pub sb_4bet_multiplier: f64,
    /// SPR below which all-in is always an option
    pub add_allin_spr: f64,
}

impl Default for PreflopBettingConfig {
    fn default() -> Self {
        Self {
            sb_open: 3.0,
            bb_3bet_multiplier: 2.5,
            sb_4bet_multiplier: 2.2,
            add_allin_spr: 7.0,
        }
    }
}

/// Postflop betting configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostflopBettingConfig {
    /// Bet sizes as fractions of pot for OOP player
    #[serde(default = "default_bet_sizes")]
    pub oop_bet_sizes: Vec<f64>,
    /// Bet sizes as fractions of pot for IP player
    #[serde(default = "default_bet_sizes")]
    pub ip_bet_sizes: Vec<f64>,
    /// Raise sizes as fractions of pot
    #[serde(default = "default_raise_sizes")]
    pub raise_sizes: Vec<f64>,
    /// SPR below which all-in is always an option
    #[serde(default = "default_allin_spr")]
    pub add_allin_spr: f64,
    /// Whether to allow donk betting
    #[serde(default)]
    pub allow_donk: bool,
    /// Maximum number of bets per street (-1 for unlimited)
    #[serde(default = "default_max_bets")]
    pub max_bets_per_street: i32,
}

fn default_bet_sizes() -> Vec<f64> {
    vec![0.66] // 66% pot
}

fn default_raise_sizes() -> Vec<f64> {
    vec![0.66]
}

fn default_allin_spr() -> f64 {
    5.0
}

fn default_max_bets() -> i32 {
    -1 // Unlimited
}

impl Default for PostflopBettingConfig {
    fn default() -> Self {
        Self {
            oop_bet_sizes: default_bet_sizes(),
            ip_bet_sizes: default_bet_sizes(),
            raise_sizes: default_raise_sizes(),
            add_allin_spr: default_allin_spr(),
            allow_donk: false,
            max_bets_per_street: default_max_bets(),
        }
    }
}

/// Card abstraction settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AbstractionSettings {
    /// Number of flop buckets
    #[serde(default = "default_flop_buckets")]
    pub flop_buckets: u16,
    /// Number of turn buckets
    #[serde(default = "default_turn_buckets")]
    pub turn_buckets: u16,
    /// Number of river buckets
    #[serde(default = "default_river_buckets")]
    pub river_buckets: u16,
    /// Number of samples for equity calculation
    #[serde(default = "default_equity_samples")]
    pub equity_samples: usize,
}

fn default_flop_buckets() -> u16 {
    1024
}

fn default_turn_buckets() -> u16 {
    256
}

fn default_river_buckets() -> u16 {
    256
}

fn default_equity_samples() -> usize {
    500
}

impl Default for AbstractionSettings {
    fn default() -> Self {
        Self {
            flop_buckets: default_flop_buckets(),
            turn_buckets: default_turn_buckets(),
            river_buckets: default_river_buckets(),
            equity_samples: default_equity_samples(),
        }
    }
}

/// Solver settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SolverSettings {
    /// Number of iterations
    #[serde(default = "default_iterations")]
    pub iterations: u64,
    /// Number of threads (0 = auto-detect)
    #[serde(default)]
    pub threads: usize,
    /// Random seed (optional)
    #[serde(default)]
    pub seed: Option<u64>,
    /// Use CFR+ algorithm
    #[serde(default = "default_true")]
    pub use_cfr_plus: bool,
    /// Use linear CFR weighting
    #[serde(default = "default_true")]
    pub use_linear_cfr: bool,
    /// Checkpoint interval (0 = no checkpoints)
    #[serde(default)]
    pub checkpoint_interval: u64,
}

fn default_iterations() -> u64 {
    100_000
}

fn default_true() -> bool {
    true
}

impl Default for SolverSettings {
    fn default() -> Self {
        Self {
            iterations: default_iterations(),
            threads: 0,
            seed: None,
            use_cfr_plus: true,
            use_linear_cfr: true,
            checkpoint_interval: 0,
        }
    }
}

impl FullGameConfig {
    /// Load configuration from a JSON file.
    pub fn from_json_file<P: AsRef<Path>>(path: P) -> Result<Self, ConfigError> {
        let content = fs::read_to_string(path.as_ref())
            .map_err(|e| ConfigError::IoError(e.to_string()))?;
        Self::from_json_str(&content)
    }

    /// Parse configuration from a JSON string.
    pub fn from_json_str(json: &str) -> Result<Self, ConfigError> {
        let config: Self = serde_json::from_str(json)
            .map_err(|e| ConfigError::ParseError(e.to_string()))?;
        config.validate()?;
        Ok(config)
    }

    /// Validate the configuration.
    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.stack_bb <= 0.0 {
            return Err(ConfigError::InvalidValue {
                field: "stack_bb".to_string(),
                message: "Stack must be positive".to_string(),
            });
        }

        if self.blinds.bb <= 0.0 || self.blinds.sb <= 0.0 {
            return Err(ConfigError::InvalidValue {
                field: "blinds".to_string(),
                message: "Blinds must be positive".to_string(),
            });
        }

        if self.blinds.sb >= self.blinds.bb {
            return Err(ConfigError::InvalidValue {
                field: "blinds".to_string(),
                message: "SB must be less than BB".to_string(),
            });
        }

        Ok(())
    }

    /// Convert to SBvsBBConfig for the game.
    pub fn to_game_config(&self) -> super::game::SBvsBBConfig {
        use super::betting::{BettingConfig, PreflopOpenSizing, Preflop3BetSizing};
        use super::abstraction::AbstractionConfig;

        super::game::SBvsBBConfig {
            stack_bb: self.stack_bb,
            sb_amount: self.blinds.sb,
            bb_amount: self.blinds.bb,
            betting: BettingConfig {
                geo_size: self.postflop.oop_bet_sizes.first().copied().unwrap_or(0.66),
                add_allin_spr: self.postflop.add_allin_spr,
                allow_donk: self.postflop.allow_donk,
                max_bets_per_street: self.postflop.max_bets_per_street,
                preflop_open: PreflopOpenSizing {
                    sb_open: self.preflop.sb_open,
                    standard_open: 2.5,
                },
                preflop_3bet: Preflop3BetSizing {
                    ip_multiplier: 2.5,
                    oop_multiplier: 3.3,
                    bb_vs_sb_multiplier: self.preflop.bb_3bet_multiplier,
                },
            },
            abstraction: AbstractionConfig {
                flop_buckets: self.abstraction.flop_buckets,
                turn_buckets: self.abstraction.turn_buckets,
                river_buckets: self.abstraction.river_buckets,
                equity_samples: self.abstraction.equity_samples,
            },
        }
    }

    /// Create a default configuration for 50bb SB vs BB.
    pub fn default_50bb() -> Self {
        Self {
            version: "1.0".to_string(),
            name: "SB vs BB 50bb".to_string(),
            description: "Default SB vs BB configuration with 50bb effective stacks".to_string(),
            stack_bb: 50.0,
            blinds: BlindsConfig::default(),
            preflop: PreflopBettingConfig::default(),
            postflop: PostflopBettingConfig::default(),
            abstraction: AbstractionSettings::default(),
            solver: SolverSettings::default(),
        }
    }

    /// Create a fast testing configuration.
    pub fn fast() -> Self {
        Self {
            version: "1.0".to_string(),
            name: "Fast Test".to_string(),
            description: "Fast configuration for testing".to_string(),
            stack_bb: 50.0,
            blinds: BlindsConfig::default(),
            preflop: PreflopBettingConfig::default(),
            postflop: PostflopBettingConfig::default(),
            abstraction: AbstractionSettings {
                flop_buckets: 100,
                turn_buckets: 50,
                river_buckets: 50,
                equity_samples: 100,
            },
            solver: SolverSettings {
                iterations: 1000,
                ..Default::default()
            },
        }
    }
}

/// Configuration error types.
#[derive(Debug, Clone)]
pub enum ConfigError {
    IoError(String),
    ParseError(String),
    InvalidValue { field: String, message: String },
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::IoError(e) => write!(f, "IO error: {}", e),
            Self::ParseError(e) => write!(f, "Parse error: {}", e),
            Self::InvalidValue { field, message } => {
                write!(f, "Invalid value for '{}': {}", field, message)
            }
        }
    }
}

impl std::error::Error for ConfigError {}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_CONFIG: &str = r#"{
        "version": "1.0",
        "name": "Test Config",
        "description": "Test configuration",
        "stack_bb": 50.0,
        "blinds": {
            "bb": 1.0,
            "sb": 0.5,
            "ante": 0.0
        },
        "preflop": {
            "sb_open": 3.0,
            "bb_3bet_multiplier": 2.5,
            "sb_4bet_multiplier": 2.2,
            "add_allin_spr": 7.0
        },
        "postflop": {
            "oop_bet_sizes": [0.66],
            "ip_bet_sizes": [0.66],
            "raise_sizes": [0.66],
            "add_allin_spr": 5.0,
            "allow_donk": false,
            "max_bets_per_street": -1
        },
        "abstraction": {
            "flop_buckets": 1024,
            "turn_buckets": 256,
            "river_buckets": 256,
            "equity_samples": 500
        },
        "solver": {
            "iterations": 100000,
            "threads": 0,
            "use_cfr_plus": true,
            "use_linear_cfr": true
        }
    }"#;

    #[test]
    fn test_parse_config() {
        let config = FullGameConfig::from_json_str(TEST_CONFIG).unwrap();

        assert_eq!(config.stack_bb, 50.0);
        assert_eq!(config.blinds.bb, 1.0);
        assert_eq!(config.blinds.sb, 0.5);
        assert_eq!(config.preflop.sb_open, 3.0);
        assert_eq!(config.abstraction.flop_buckets, 1024);
    }

    #[test]
    fn test_default_config() {
        let config = FullGameConfig::default_50bb();

        assert_eq!(config.stack_bb, 50.0);
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_to_game_config() {
        let config = FullGameConfig::default_50bb();
        let game_config = config.to_game_config();

        assert_eq!(game_config.stack_bb, 50.0);
        assert_eq!(game_config.sb_amount, 0.5);
        assert_eq!(game_config.bb_amount, 1.0);
    }

    #[test]
    fn test_validation_fails() {
        let mut config = FullGameConfig::default_50bb();
        config.stack_bb = -10.0;

        assert!(config.validate().is_err());
    }

    #[test]
    fn test_minimal_config() {
        // Test with minimal required fields
        let minimal = r#"{
            "version": "1.0",
            "name": "Minimal",
            "stack_bb": 50.0,
            "blinds": { "bb": 1.0, "sb": 0.5 },
            "preflop": {
                "sb_open": 3.0,
                "bb_3bet_multiplier": 2.5,
                "sb_4bet_multiplier": 2.2,
                "add_allin_spr": 7.0
            },
            "postflop": {},
            "abstraction": {}
        }"#;

        let config = FullGameConfig::from_json_str(minimal).unwrap();
        assert_eq!(config.stack_bb, 50.0);
        // Defaults should be applied
        assert_eq!(config.abstraction.flop_buckets, 1024);
        assert_eq!(config.postflop.oop_bet_sizes, vec![0.66]);
    }
}
