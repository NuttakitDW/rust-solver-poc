//! Solution output and export utilities.
//!
//! This module provides functions for exporting CFR solver results
//! to various formats for analysis and visualization.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::File;
use std::io::Write;
use std::path::Path;

use crate::cfr::CFRSolver;
use super::game::SBvsBBFullGame;
use super::abstraction::HandClass;

/// Strategy entry for a single info state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategyEntry {
    /// Info state key
    pub info_key: String,
    /// Position (0=SB, 1=BB)
    pub position: u8,
    /// Street (0=Preflop, 1=Flop, 2=Turn, 3=River)
    pub street: u8,
    /// Hand bucket
    pub bucket: u16,
    /// Action history
    pub history: String,
    /// Action names
    pub actions: Vec<String>,
    /// Strategy probabilities
    pub strategy: Vec<f64>,
}

/// Complete solver output.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SolverOutput {
    /// Solver metadata
    pub metadata: SolverMetadata,
    /// Strategies indexed by info state key
    pub strategies: HashMap<String, StrategyEntry>,
}

/// Solver metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SolverMetadata {
    /// Configuration name
    pub config_name: String,
    /// Stack depth
    pub stack_bb: f64,
    /// Number of iterations
    pub iterations: u64,
    /// Number of info sets discovered
    pub num_info_sets: usize,
    /// Timestamp
    pub timestamp: String,
}

impl SolverOutput {
    /// Create output from a solver.
    pub fn from_solver(
        solver: &CFRSolver<SBvsBBFullGame>,
        config_name: &str,
        stack_bb: f64,
    ) -> Self {
        let mut strategies = HashMap::new();

        // Extract all strategies from the solver
        for key in solver.info_set_keys() {
            if let Some(entry) = Self::parse_key_and_get_strategy(solver, &key) {
                strategies.insert(key, entry);
            }
        }

        Self {
            metadata: SolverMetadata {
                config_name: config_name.to_string(),
                stack_bb,
                iterations: solver.iteration(),
                num_info_sets: solver.num_info_sets(),
                timestamp: chrono_lite_timestamp(),
            },
            strategies,
        }
    }

    /// Parse an info state key and get the strategy.
    fn parse_key_and_get_strategy(
        solver: &CFRSolver<SBvsBBFullGame>,
        key: &str,
    ) -> Option<StrategyEntry> {
        // Key format: P{position}S{street}B{bucket}|{history}
        // Example: P0S1B523|R300-C|X-B132

        if !key.starts_with('P') {
            return None;
        }

        let parts: Vec<&str> = key.splitn(2, '|').collect();
        if parts.is_empty() {
            return None;
        }

        let header = parts[0];
        let history = parts.get(1).map(|s| s.to_string()).unwrap_or_default();

        // Parse position, street, bucket from header
        let position = header.get(1..2)?.parse::<u8>().ok()?;

        // Find S and B markers
        let s_pos = header.find('S')?;
        let b_pos = header.find('B')?;

        let street = header.get(s_pos + 1..b_pos)?.parse::<u8>().ok()?;
        let bucket = header.get(b_pos + 1..)?.parse::<u16>().ok()?;

        // Get strategy from solver (we don't know exact num_actions, try common values)
        let strategy = solver.get_average_strategy(key, 10);

        // Remove trailing zeros
        let mut strategy: Vec<f64> = strategy.into_iter()
            .rev()
            .skip_while(|&x| x == 0.0)
            .collect();
        strategy.reverse();

        if strategy.is_empty() {
            return None;
        }

        // Generate action names (simplified)
        let actions: Vec<String> = (0..strategy.len())
            .map(|i| format!("action_{}", i))
            .collect();

        Some(StrategyEntry {
            info_key: key.to_string(),
            position,
            street,
            bucket,
            history,
            actions,
            strategy,
        })
    }

    /// Save to JSON file.
    pub fn save_json<P: AsRef<Path>>(&self, path: P) -> std::io::Result<()> {
        let json = serde_json::to_string_pretty(self)?;
        let mut file = File::create(path)?;
        file.write_all(json.as_bytes())
    }

    /// Get strategy for a specific info state.
    pub fn get_strategy(&self, key: &str) -> Option<&StrategyEntry> {
        self.strategies.get(key)
    }

    /// Get preflop strategies for a position.
    pub fn preflop_strategies(&self, position: u8) -> Vec<&StrategyEntry> {
        self.strategies.values()
            .filter(|e| e.position == position && e.street == 0)
            .collect()
    }
}

/// Preflop range output grouped by hand class.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreflopRangeOutput {
    /// Position (SB or BB)
    pub position: String,
    /// Action scenario (e.g., "open", "vs_3bet")
    pub scenario: String,
    /// Range by hand class
    pub range: HashMap<String, HandClassStrategy>,
}

/// Strategy for a hand class.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HandClassStrategy {
    /// Hand class name (e.g., "AA", "AKs", "72o")
    pub hand: String,
    /// Action frequencies
    pub frequencies: HashMap<String, f64>,
}

impl PreflopRangeOutput {
    /// Create preflop range from solver output.
    pub fn from_solver_output(
        output: &SolverOutput,
        position: u8,
        history_filter: &str,
    ) -> Self {
        let position_str = if position == 0 { "SB" } else { "BB" };
        let mut range = HashMap::new();

        // Group by bucket (which corresponds to hand class for preflop)
        for entry in output.strategies.values() {
            if entry.position != position || entry.street != 0 {
                continue;
            }

            // Filter by history if specified
            if !history_filter.is_empty() && !entry.history.starts_with(history_filter) {
                continue;
            }

            // Get hand class name from bucket
            let hand_class = HandClass::from_index(entry.bucket as u8);
            let hand_name = hand_class.to_string();

            // Convert strategy to frequencies
            let mut frequencies = HashMap::new();
            for (i, &prob) in entry.strategy.iter().enumerate() {
                let action_name = entry.actions.get(i)
                    .cloned()
                    .unwrap_or_else(|| format!("action_{}", i));
                frequencies.insert(action_name, prob);
            }

            range.insert(hand_name.clone(), HandClassStrategy {
                hand: hand_name,
                frequencies,
            });
        }

        Self {
            position: position_str.to_string(),
            scenario: if history_filter.is_empty() {
                "open".to_string()
            } else {
                history_filter.to_string()
            },
            range,
        }
    }

    /// Get opening frequency for a hand.
    pub fn get_open_frequency(&self, hand: &str) -> Option<f64> {
        self.range.get(hand)
            .and_then(|hs| hs.frequencies.get("Raise")
                .or(hs.frequencies.get("action_2"))  // Raise is usually action 2
                .copied())
    }

    /// Save to JSON file.
    pub fn save_json<P: AsRef<Path>>(&self, path: P) -> std::io::Result<()> {
        let json = serde_json::to_string_pretty(self)?;
        let mut file = File::create(path)?;
        file.write_all(json.as_bytes())
    }
}

/// Simple timestamp without external dependencies.
fn chrono_lite_timestamp() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};

    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();

    format!("{}", duration.as_secs())
}

/// Export solver results to CSV format.
pub fn export_csv<P: AsRef<Path>>(
    output: &SolverOutput,
    path: P,
) -> std::io::Result<()> {
    let mut file = File::create(path)?;

    // Header
    writeln!(file, "info_key,position,street,bucket,history,action,probability")?;

    // Data rows
    for entry in output.strategies.values() {
        for (i, prob) in entry.strategy.iter().enumerate() {
            let action = entry.actions.get(i)
                .cloned()
                .unwrap_or_else(|| format!("action_{}", i));

            writeln!(file, "{},{},{},{},{},{},{:.6}",
                entry.info_key,
                entry.position,
                entry.street,
                entry.bucket,
                entry.history,
                action,
                prob
            )?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cfr::CFRConfig;

    #[test]
    fn test_solver_output_creation() {
        let game = SBvsBBFullGame::fast();
        let config = CFRConfig::default().with_seed(42);
        let mut solver = CFRSolver::new(game, config);

        // Run some iterations
        solver.train(100);

        // Create output
        let output = SolverOutput::from_solver(&solver, "test", 50.0);

        assert_eq!(output.metadata.config_name, "test");
        assert_eq!(output.metadata.stack_bb, 50.0);
        assert_eq!(output.metadata.iterations, 100);
    }

    #[test]
    fn test_parse_info_key() {
        // Test key parsing
        let key = "P0S1B523|R300-C|X-B132";
        let parts: Vec<&str> = key.splitn(2, '|').collect();

        assert_eq!(parts.len(), 2);
        assert_eq!(parts[0], "P0S1B523");
        assert_eq!(parts[1], "R300-C|X-B132");

        // Parse header
        let header = parts[0];
        assert!(header.starts_with("P0"));
        assert!(header.contains("S1"));
        assert!(header.contains("B523"));
    }
}
