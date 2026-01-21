//! Preflop game tree configuration loaded from JSON.
//!
//! This module provides configuration structs that deserialize from JSON config files,
//! enabling game tree settings to be modified without recompiling.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

/// Position labels for up to 10-max game.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum Position {
    UTG,
    #[serde(alias = "UTG1", alias = "UTG+1")]
    EP,
    #[serde(alias = "UTG2", alias = "UTG+2", alias = "LJ")]
    MP,
    HJ,
    CO,
    BU,
    SB,
    BB,
}

impl Position {
    /// Get all positions in preflop action order for 8-max.
    pub fn all_8max() -> &'static [Position] {
        &[
            Position::UTG,
            Position::EP,
            Position::MP,
            Position::HJ,
            Position::CO,
            Position::BU,
            Position::SB,
            Position::BB,
        ]
    }

    /// Check if this position is in position (IP) vs another postflop.
    pub fn is_ip_vs(&self, other: &Position) -> bool {
        let order = Self::all_8max();
        let self_idx = order.iter().position(|p| p == self);
        let other_idx = order.iter().position(|p| p == other);
        match (self_idx, other_idx) {
            (Some(s), Some(o)) => s > o,
            _ => false,
        }
    }
}

// ============================================================================
// JSON Config Structures (match the JSON schema exactly)
// ============================================================================

/// Root configuration structure matching JSON schema.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreflopConfig {
    pub version: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    pub hand_data: HandData,
    pub blinds: Blinds,
    pub equity_model: EquityModel,
    pub action_restrictions: ActionRestrictions,
    pub sizing: Sizing,
    /// Optional scenario filter - if present, only solve specified spots
    #[serde(default)]
    pub scenarios: Option<ScenarioFilter>,
}

/// Hand data configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HandData {
    pub num_players: u8,
    pub positions: Vec<String>,
    pub stacks: HashMap<String, f64>,
    /// Skip small blind position (some casino rules)
    #[serde(default)]
    pub skip_sb: bool,
    /// Moving button rule
    #[serde(default)]
    pub moving_bu: bool,
    /// Straddle type: "OFF", "MANDATORY", "VOLUNTARY"
    #[serde(default = "default_straddle_type")]
    pub straddle_type: String,
}

fn default_straddle_type() -> String {
    "OFF".to_string()
}

/// Blinds configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Blinds {
    pub bb: f64,
    pub sb: f64,
    #[serde(default)]
    pub ante: f64,
    #[serde(default = "default_ante_type")]
    pub ante_type: String,
}

fn default_ante_type() -> String {
    "REGULAR".to_string()
}

/// Equity model configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EquityModel {
    #[serde(rename = "type")]
    pub model_type: String,
    #[serde(default)]
    pub raked: bool,
}

/// Action restrictions configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionRestrictions {
    /// [RFI, 2bet, 3bet, 4bet, 5bet] - number of flats allowed at each level
    pub allowed_flats_per_raise: [u8; 5],
    pub allow_cold_calls: bool,
    pub allow_flats_closing_action: bool,
    pub allow_sb_complete: bool,
    pub preflop_add_allin_spr: f64,
    pub preflop_allin_threshold: f64,
}

/// All sizing configurations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Sizing {
    pub open: OpenSizing,
    pub threebet: ThreebetSizing,
    pub fourbet: BetLevelSizing,
    pub fivebet: BetLevelSizing,
}

/// Open raise sizing by position.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenSizing {
    pub others: BetSize,
    pub bu: BetSize,
    pub sb: BetSize,
    pub bb: BetSize,
    pub bb_vs_sb: BetSize,
}

/// 3-bet sizing by position/situation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThreebetSizing {
    pub ip: BetSize,
    pub bb_vs_sb: BetSize,
    pub bb_vs_other: BetSize,
    pub sb_vs_bb: BetSize,
    pub sb_vs_other: BetSize,
}

/// 4-bet/5-bet sizing (IP and OOP).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BetLevelSizing {
    pub ip: PotPercentSize,
    pub oop: PotPercentSize,
}

/// Bet size with base and per-caller adjustment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BetSize {
    pub base: f64,
    #[serde(default)]
    pub per_caller: f64,
}

impl BetSize {
    pub fn new(base: f64, per_caller: f64) -> Self {
        Self { base, per_caller }
    }

    /// Calculate actual size given number of callers.
    pub fn calculate(&self, num_callers: usize) -> f64 {
        self.base + self.per_caller * num_callers as f64
    }
}

/// Pot percentage sizing for 4bet/5bet.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PotPercentSize {
    pub percent_pot: f64,
    #[serde(default)]
    pub include_allin: bool,
}

// ============================================================================
// Scenario Filtering
// ============================================================================

/// Filter to solve only specific spots/scenarios.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScenarioFilter {
    /// List of specific spots to solve
    pub spots: Vec<Spot>,
}

/// A specific spot to solve (e.g., BB vs UTG RFI).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Spot {
    /// The position that opens (RFI)
    pub rfi: String,
    /// The position that defends/responds
    pub defender: String,
    /// Optional: include 3bet pots (default: true)
    #[serde(default = "default_true")]
    pub include_3bet: bool,
    /// Optional: include 4bet+ pots (default: true)
    #[serde(default = "default_true")]
    pub include_4bet_plus: bool,
}

fn default_true() -> bool {
    true
}

impl ScenarioFilter {
    /// Check if a spot (rfi_position, defender_position) should be included.
    pub fn includes_spot(&self, rfi: &str, defender: &str) -> bool {
        self.spots.iter().any(|s| s.rfi == rfi && s.defender == defender)
    }

    /// Get the spot config for a specific matchup.
    pub fn get_spot(&self, rfi: &str, defender: &str) -> Option<&Spot> {
        self.spots.iter().find(|s| s.rfi == rfi && s.defender == defender)
    }
}

// ============================================================================
// Config Loading and Validation
// ============================================================================

impl PreflopConfig {
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
        // Check player count
        if self.hand_data.num_players < 2 || self.hand_data.num_players > 10 {
            return Err(ConfigError::InvalidPlayerCount(self.hand_data.num_players));
        }

        // Check positions match player count
        if self.hand_data.positions.len() != self.hand_data.num_players as usize {
            return Err(ConfigError::PositionCountMismatch {
                expected: self.hand_data.num_players as usize,
                actual: self.hand_data.positions.len(),
            });
        }

        // Check all positions have stacks
        for pos in &self.hand_data.positions {
            if !self.hand_data.stacks.contains_key(pos) {
                return Err(ConfigError::MissingStack(pos.clone()));
            }
            let stack = self.hand_data.stacks[pos];
            if stack <= 0.0 {
                return Err(ConfigError::InvalidStack {
                    position: pos.clone(),
                    stack,
                });
            }
        }

        // Check blinds are positive
        if self.blinds.bb <= 0.0 || self.blinds.sb <= 0.0 {
            return Err(ConfigError::InvalidBlinds {
                bb: self.blinds.bb,
                sb: self.blinds.sb,
            });
        }

        Ok(())
    }

    /// Get stack for a position by name.
    pub fn stack_for(&self, position: &str) -> Option<f64> {
        self.hand_data.stacks.get(position).copied()
    }

    /// Calculate starting pot (blinds + antes).
    pub fn starting_pot(&self) -> f64 {
        self.blinds.bb + self.blinds.sb + (self.blinds.ante * self.hand_data.num_players as f64)
    }

    /// Check if a position can flat at a given raise level.
    /// Level: 0=RFI, 1=facing RFI, 2=facing 3bet, 3=facing 4bet, 4=facing 5bet
    pub fn can_flat_at_level(&self, level: usize, current_flats: u8) -> bool {
        if level >= 5 {
            return false;
        }
        current_flats < self.action_restrictions.allowed_flats_per_raise[level]
    }

    /// Get open sizing for a position.
    pub fn get_open_sizing(&self, position: &str) -> &BetSize {
        match position {
            "BU" => &self.sizing.open.bu,
            "SB" => &self.sizing.open.sb,
            "BB" => &self.sizing.open.bb,
            _ => &self.sizing.open.others,
        }
    }

    /// Get 3bet sizing based on raiser position, 3bettor position, and IP/OOP status.
    pub fn get_3bet_sizing(&self, raiser: &str, three_bettor: &str) -> &BetSize {
        match (three_bettor, raiser) {
            ("BB", "SB") => &self.sizing.threebet.bb_vs_sb,
            ("BB", _) => &self.sizing.threebet.bb_vs_other,
            ("SB", "BB") => &self.sizing.threebet.sb_vs_bb,
            ("SB", _) => &self.sizing.threebet.sb_vs_other,
            _ => &self.sizing.threebet.ip, // IP 3bet for other positions
        }
    }

    /// Get 4bet/5bet sizing based on IP/OOP status.
    pub fn get_4bet_sizing(&self, is_ip: bool) -> &PotPercentSize {
        if is_ip {
            &self.sizing.fourbet.ip
        } else {
            &self.sizing.fourbet.oop
        }
    }

    pub fn get_5bet_sizing(&self, is_ip: bool) -> &PotPercentSize {
        if is_ip {
            &self.sizing.fivebet.ip
        } else {
            &self.sizing.fivebet.oop
        }
    }

    /// Check if all-in should be included as an option.
    pub fn should_include_allin(&self, stack: f64, pot: f64) -> bool {
        let spr = stack / pot;
        stack <= self.action_restrictions.preflop_allin_threshold
            || spr <= self.action_restrictions.preflop_add_allin_spr
    }

    /// Check if a specific spot should be solved.
    /// Returns true if no filter is set (solve all) or if the spot is in the filter.
    pub fn should_solve_spot(&self, rfi: &str, defender: &str) -> bool {
        match &self.scenarios {
            None => true, // No filter = solve all spots
            Some(filter) => filter.includes_spot(rfi, defender),
        }
    }

    /// Get spot config if filtering is enabled.
    pub fn get_spot_config(&self, rfi: &str, defender: &str) -> Option<&Spot> {
        self.scenarios.as_ref()?.get_spot(rfi, defender)
    }

    /// Get list of all spots to solve based on filter.
    /// If no filter, returns all possible RFI vs Defender combinations.
    pub fn spots_to_solve(&self) -> Vec<(String, String)> {
        match &self.scenarios {
            Some(filter) => filter.spots.iter()
                .map(|s| (s.rfi.clone(), s.defender.clone()))
                .collect(),
            None => {
                // Generate all possible spots
                let mut spots = Vec::new();
                let positions = &self.hand_data.positions;
                for (i, rfi) in positions.iter().enumerate() {
                    // RFI can be any position except BB (who can't RFI by definition)
                    if rfi == "BB" {
                        continue;
                    }
                    // Defenders are positions after the RFI
                    for defender in positions.iter().skip(i + 1) {
                        spots.push((rfi.clone(), defender.clone()));
                    }
                }
                spots
            }
        }
    }
}

// ============================================================================
// Errors
// ============================================================================

#[derive(Debug, Clone)]
pub enum ConfigError {
    IoError(String),
    ParseError(String),
    InvalidPlayerCount(u8),
    PositionCountMismatch { expected: usize, actual: usize },
    MissingStack(String),
    InvalidStack { position: String, stack: f64 },
    InvalidBlinds { bb: f64, sb: f64 },
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::IoError(e) => write!(f, "IO error: {}", e),
            Self::ParseError(e) => write!(f, "Parse error: {}", e),
            Self::InvalidPlayerCount(n) => write!(f, "Invalid player count: {} (must be 2-10)", n),
            Self::PositionCountMismatch { expected, actual } => {
                write!(f, "Position count mismatch: expected {}, got {}", expected, actual)
            }
            Self::MissingStack(pos) => write!(f, "Missing stack for position: {}", pos),
            Self::InvalidStack { position, stack } => {
                write!(f, "Invalid stack {} for position {}", stack, position)
            }
            Self::InvalidBlinds { bb, sb } => {
                write!(f, "Invalid blinds: BB={}, SB={}", bb, sb)
            }
        }
    }
}

impl std::error::Error for ConfigError {}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_CONFIG: &str = r#"{
        "version": "1.0",
        "name": "Test Config",
        "description": "Test",
        "hand_data": {
            "num_players": 8,
            "positions": ["UTG", "EP", "MP", "HJ", "CO", "BU", "SB", "BB"],
            "stacks": {
                "UTG": 50.0, "EP": 50.0, "MP": 50.0, "HJ": 50.0,
                "CO": 50.0, "BU": 50.0, "SB": 50.0, "BB": 50.0
            }
        },
        "blinds": { "bb": 1.0, "sb": 0.5, "ante": 0.12, "ante_type": "REGULAR" },
        "equity_model": { "type": "ChipEV", "raked": false },
        "action_restrictions": {
            "allowed_flats_per_raise": [0, 1, 1, 1, 0],
            "allow_cold_calls": false,
            "allow_flats_closing_action": true,
            "allow_sb_complete": true,
            "preflop_add_allin_spr": 7.0,
            "preflop_allin_threshold": 40.0
        },
        "sizing": {
            "open": {
                "others": { "base": 2.3, "per_caller": 1.0 },
                "bu": { "base": 2.3, "per_caller": 1.0 },
                "sb": { "base": 3.5, "per_caller": 1.0 },
                "bb": { "base": 3.5, "per_caller": 1.0 },
                "bb_vs_sb": { "base": 3.0, "per_caller": 0.0 }
            },
            "threebet": {
                "ip": { "base": 2.5, "per_caller": 1.0 },
                "bb_vs_sb": { "base": 2.5, "per_caller": 0.0 },
                "bb_vs_other": { "base": 3.3, "per_caller": 1.0 },
                "sb_vs_bb": { "base": 2.6, "per_caller": 1.0 },
                "sb_vs_other": { "base": 3.3, "per_caller": 1.0 }
            },
            "fourbet": {
                "ip": { "percent_pot": 0.90, "include_allin": true },
                "oop": { "percent_pot": 1.20, "include_allin": true }
            },
            "fivebet": {
                "ip": { "percent_pot": 0.90, "include_allin": true },
                "oop": { "percent_pot": 1.20, "include_allin": true }
            }
        }
    }"#;

    #[test]
    fn test_parse_json_config() {
        let config = PreflopConfig::from_json_str(TEST_CONFIG).unwrap();

        assert_eq!(config.hand_data.num_players, 8);
        assert_eq!(config.blinds.bb, 1.0);
        assert_eq!(config.blinds.sb, 0.5);
        assert_eq!(config.blinds.ante, 0.12);
    }

    #[test]
    fn test_starting_pot() {
        let config = PreflopConfig::from_json_str(TEST_CONFIG).unwrap();
        // 1.0 + 0.5 + (0.12 * 8) = 2.46
        assert!((config.starting_pot() - 2.46).abs() < 0.001);
    }

    #[test]
    fn test_get_open_sizing() {
        let config = PreflopConfig::from_json_str(TEST_CONFIG).unwrap();

        assert_eq!(config.get_open_sizing("UTG").base, 2.3);
        assert_eq!(config.get_open_sizing("BU").base, 2.3);
        assert_eq!(config.get_open_sizing("SB").base, 3.5);
    }

    #[test]
    fn test_get_3bet_sizing() {
        let config = PreflopConfig::from_json_str(TEST_CONFIG).unwrap();

        assert_eq!(config.get_3bet_sizing("UTG", "BB").base, 3.3); // BB vs other
        assert_eq!(config.get_3bet_sizing("SB", "BB").base, 2.5);  // BB vs SB
        assert_eq!(config.get_3bet_sizing("UTG", "CO").base, 2.5); // IP 3bet
    }

    #[test]
    fn test_can_flat_at_level() {
        let config = PreflopConfig::from_json_str(TEST_CONFIG).unwrap();

        // Level 0 (RFI): no flats allowed
        assert!(!config.can_flat_at_level(0, 0));

        // Level 1 (facing RFI): 1 flat allowed
        assert!(config.can_flat_at_level(1, 0));
        assert!(!config.can_flat_at_level(1, 1));

        // Level 4 (facing 5bet): no flats allowed
        assert!(!config.can_flat_at_level(4, 0));
    }

    #[test]
    fn test_should_include_allin() {
        let config = PreflopConfig::from_json_str(TEST_CONFIG).unwrap();

        // Stack <= 40bb threshold
        assert!(config.should_include_allin(35.0, 5.0));

        // SPR <= 7.0
        assert!(config.should_include_allin(50.0, 10.0)); // SPR = 5.0

        // Neither condition met
        assert!(!config.should_include_allin(50.0, 2.0)); // SPR = 25.0
    }

    #[test]
    fn test_scenario_filter() {
        let config_with_filter = PreflopConfig::from_json_file("configs/bb_vs_utg_50bb.json").unwrap();

        // Should include BB vs UTG
        assert!(config_with_filter.should_solve_spot("UTG", "BB"));

        // Should NOT include other spots
        assert!(!config_with_filter.should_solve_spot("BU", "BB"));
        assert!(!config_with_filter.should_solve_spot("UTG", "SB"));
        assert!(!config_with_filter.should_solve_spot("CO", "BB"));

        // spots_to_solve should only return the filtered spot
        let spots = config_with_filter.spots_to_solve();
        assert_eq!(spots.len(), 1);
        assert_eq!(spots[0], ("UTG".to_string(), "BB".to_string()));
    }

    #[test]
    fn test_no_filter_solves_all_spots() {
        let config = PreflopConfig::from_json_file("configs/preflop_8max_50bb.json").unwrap();

        // Without filter, all spots should be solved
        assert!(config.should_solve_spot("UTG", "BB"));
        assert!(config.should_solve_spot("BU", "BB"));
        assert!(config.should_solve_spot("CO", "SB"));

        // Should generate all valid spots
        let spots = config.spots_to_solve();
        // UTG can face: EP, MP, HJ, CO, BU, SB, BB = 7 defenders
        // EP can face: MP, HJ, CO, BU, SB, BB = 6 defenders
        // ... etc
        // Total: 7 + 6 + 5 + 4 + 3 + 2 + 1 = 28 spots
        assert_eq!(spots.len(), 28);
    }

    #[test]
    fn test_load_hrc_config_file() {
        // Test loading the actual HRC config file
        let config = PreflopConfig::from_json_file("configs/preflop_8max_50bb.json").unwrap();

        // Verify all HRC settings are correct
        assert_eq!(config.hand_data.num_players, 8);
        assert_eq!(config.blinds.bb, 1.0);
        assert_eq!(config.blinds.sb, 0.5);
        assert_eq!(config.blinds.ante, 0.12);
        assert_eq!(config.blinds.ante_type, "REGULAR");

        // Hand data edge-case settings
        assert!(!config.hand_data.skip_sb);
        assert!(!config.hand_data.moving_bu);
        assert_eq!(config.hand_data.straddle_type, "OFF");

        // Equity model
        assert_eq!(config.equity_model.model_type, "ChipEV");
        assert!(!config.equity_model.raked);

        // Action restrictions
        assert_eq!(config.action_restrictions.allowed_flats_per_raise, [0, 1, 1, 1, 0]);
        assert!(!config.action_restrictions.allow_cold_calls);
        assert!(config.action_restrictions.allow_flats_closing_action);
        assert!(config.action_restrictions.allow_sb_complete);
        assert_eq!(config.action_restrictions.preflop_add_allin_spr, 7.0);
        assert_eq!(config.action_restrictions.preflop_allin_threshold, 40.0);

        // Open sizing
        assert_eq!(config.sizing.open.others.base, 2.3);
        assert_eq!(config.sizing.open.others.per_caller, 1.0);
        assert_eq!(config.sizing.open.bu.base, 2.3);
        assert_eq!(config.sizing.open.sb.base, 3.5);
        assert_eq!(config.sizing.open.bb.base, 3.5);
        assert_eq!(config.sizing.open.bb_vs_sb.base, 3.0);

        // 3bet sizing
        assert_eq!(config.sizing.threebet.ip.base, 2.5);
        assert_eq!(config.sizing.threebet.bb_vs_sb.base, 2.5);
        assert_eq!(config.sizing.threebet.bb_vs_other.base, 3.3);
        assert_eq!(config.sizing.threebet.sb_vs_bb.base, 2.6);
        assert_eq!(config.sizing.threebet.sb_vs_other.base, 3.3);

        // 4bet/5bet sizing
        assert_eq!(config.sizing.fourbet.ip.percent_pot, 0.90);
        assert_eq!(config.sizing.fourbet.oop.percent_pot, 1.20);
        assert!(config.sizing.fourbet.ip.include_allin);
        assert_eq!(config.sizing.fivebet.ip.percent_pot, 0.90);
        assert_eq!(config.sizing.fivebet.oop.percent_pot, 1.20);
    }

    #[test]
    fn test_validation_fails_for_invalid_config() {
        let invalid = r#"{
            "version": "1.0",
            "name": "Invalid",
            "hand_data": {
                "num_players": 15,
                "positions": [],
                "stacks": {}
            },
            "blinds": { "bb": 1.0, "sb": 0.5 },
            "equity_model": { "type": "ChipEV" },
            "action_restrictions": {
                "allowed_flats_per_raise": [0, 1, 1, 1, 0],
                "allow_cold_calls": false,
                "allow_flats_closing_action": true,
                "allow_sb_complete": true,
                "preflop_add_allin_spr": 7.0,
                "preflop_allin_threshold": 40.0
            },
            "sizing": {
                "open": {
                    "others": { "base": 2.3 },
                    "bu": { "base": 2.3 },
                    "sb": { "base": 3.5 },
                    "bb": { "base": 3.5 },
                    "bb_vs_sb": { "base": 3.0 }
                },
                "threebet": {
                    "ip": { "base": 2.5 },
                    "bb_vs_sb": { "base": 2.5 },
                    "bb_vs_other": { "base": 3.3 },
                    "sb_vs_bb": { "base": 2.6 },
                    "sb_vs_other": { "base": 3.3 }
                },
                "fourbet": {
                    "ip": { "percent_pot": 0.90 },
                    "oop": { "percent_pot": 1.20 }
                },
                "fivebet": {
                    "ip": { "percent_pot": 0.90 },
                    "oop": { "percent_pot": 1.20 }
                }
            }
        }"#;

        let result = PreflopConfig::from_json_str(invalid);
        assert!(result.is_err());
    }
}
