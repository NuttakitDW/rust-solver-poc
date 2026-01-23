//! 8-max preflop solver binary.
//!
//! Solves all 8-max preflop positions with HRC-compatible settings.
//! Target: < 2 minutes for full solution.

use std::time::Instant;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::collections::HashMap;
use std::fs::File;
use std::io::Write;

use rayon::prelude::*;
use serde::{Serialize, Deserialize};

use rust_solver_poc::cfr::{CFRConfig, CFRSolver};
use rust_solver_poc::games::preflop_8max::{Preflop8MaxGame, Preflop8MaxConfig};
use rust_solver_poc::games::preflop::config::PreflopConfig;

fn main() {
    println!("=== 8-Max Preflop Solver ===");
    println!("Target: HRC-equivalent solution in < 2 minutes\n");

    let total_start = Instant::now();

    // Load config or use defaults
    let config = load_config();
    println!("Configuration: {}", config.name);
    println!("Stack: {}bb, Ante: {}bb",
        config.hand_data.stacks.values().next().unwrap_or(&50.0),
        config.blinds.ante);
    println!();

    // Get all spots to solve
    let spots = config.spots_to_solve();
    println!("Spots to solve: {}", spots.len());

    // Solve all spots in parallel
    let completed = Arc::new(AtomicUsize::new(0));
    let total_spots = spots.len();

    let results: Vec<SpotResult> = spots.par_iter()
        .map(|(rfi, defender)| {
            let spot_start = Instant::now();

            // Create game for this spot
            let game_config = create_spot_config(&config, rfi, defender);
            let game = Preflop8MaxGame::with_config(game_config);

            // Configure solver
            let solver_config = CFRConfig::default()
                .with_cfr_plus(true)
                .with_linear_cfr(true)
                .with_exploration(0.3);

            let mut solver = CFRSolver::new(game, solver_config);

            // Train until convergence
            let ci_target = 10.0;
            let result = solver.train_parallel_until_converged(
                ci_target,
                1000,  // batch size
                100000, // max iterations
                0,      // auto-detect threads
                None::<fn(&_)>,
            );

            let elapsed = spot_start.elapsed();
            let count = completed.fetch_add(1, Ordering::Relaxed) + 1;

            println!("[{}/{}] {} vs {} - CI: {:.2}, iters: {}, time: {:.2}s",
                count, total_spots, rfi, defender,
                result.final_ci, result.iterations, elapsed.as_secs_f64());

            SpotResult {
                rfi: rfi.clone(),
                defender: defender.clone(),
                ci: result.final_ci,
                iterations: result.iterations,
                elapsed_secs: elapsed.as_secs_f64(),
                info_sets: solver.num_info_sets(),
            }
        })
        .collect();

    let total_elapsed = total_start.elapsed();

    // Print summary
    println!("\n=== Summary ===");
    println!("Total time: {:.2}s", total_elapsed.as_secs_f64());
    println!("Spots solved: {}", results.len());

    let avg_ci: f64 = results.iter().map(|r| r.ci).sum::<f64>() / results.len() as f64;
    let total_info_sets: usize = results.iter().map(|r| r.info_sets).sum();
    let total_iterations: u64 = results.iter().map(|r| r.iterations).sum();

    println!("Average CI: {:.2}", avg_ci);
    println!("Total info sets: {}", total_info_sets);
    println!("Total iterations: {}", total_iterations);

    if total_elapsed.as_secs_f64() < 120.0 {
        println!("\n SUCCESS: Solved in under 2 minutes!");
    } else {
        println!("\n Target missed: {:.1}s > 120s", total_elapsed.as_secs_f64());
    }

    // Export solution
    export_solution(&results, &config, total_elapsed.as_secs_f64());
}

fn load_config() -> PreflopConfig {
    // Try to load from file first
    let config_path = std::env::args().nth(1)
        .unwrap_or_else(|| "configs/hrc_8max_50bb.json".to_string());

    match PreflopConfig::from_json_file(&config_path) {
        Ok(config) => {
            println!("Loaded config from: {}", config_path);
            config
        }
        Err(e) => {
            println!("Could not load {}: {}", config_path, e);
            println!("Using default HRC-equivalent settings");
            create_default_config()
        }
    }
}

fn create_default_config() -> PreflopConfig {
    // Create HRC-equivalent config programmatically
    use rust_solver_poc::games::preflop::config::*;
    use std::collections::HashMap;

    let mut stacks = HashMap::new();
    for pos in ["UTG", "EP", "MP", "HJ", "CO", "BU", "SB", "BB"] {
        stacks.insert(pos.to_string(), 50.0);
    }

    PreflopConfig {
        version: "1.0".to_string(),
        name: "HRC 8-max 50bb Default".to_string(),
        description: "Default HRC-equivalent settings".to_string(),
        hand_data: HandData {
            num_players: 8,
            positions: vec!["UTG", "EP", "MP", "HJ", "CO", "BU", "SB", "BB"]
                .into_iter().map(String::from).collect(),
            stacks,
            skip_sb: false,
            moving_bu: false,
            straddle_type: "OFF".to_string(),
        },
        blinds: Blinds {
            bb: 1.0,
            sb: 0.5,
            ante: 0.12,
            ante_type: "REGULAR".to_string(),
        },
        equity_model: EquityModel {
            model_type: "ChipEV".to_string(),
            raked: false,
        },
        action_restrictions: ActionRestrictions {
            allowed_flats_per_raise: [0, 1, 1, 1, 0],
            allow_cold_calls: false,
            allow_flats_closing_action: true,
            allow_sb_complete: true,
            preflop_add_allin_spr: 7.0,
            preflop_allin_threshold: 40.0,
        },
        sizing: Sizing {
            open: OpenSizing {
                others: BetSize::new(2.3, 1.0),
                bu: BetSize::new(2.3, 1.0),
                sb: BetSize::new(3.5, 1.0),
                bb: BetSize::new(3.5, 1.0),
                bb_vs_sb: BetSize::new(3.0, 0.0),
            },
            threebet: ThreebetSizing {
                ip: BetSize::new(2.5, 1.0),
                bb_vs_sb: BetSize::new(2.5, 0.0),
                bb_vs_other: BetSize::new(3.3, 1.0),
                sb_vs_bb: BetSize::new(2.6, 1.0),
                sb_vs_other: BetSize::new(3.3, 1.0),
            },
            fourbet: BetLevelSizing {
                ip: PotPercentSize { percent_pot: 0.90, include_allin: true },
                oop: PotPercentSize { percent_pot: 1.20, include_allin: true },
            },
            fivebet: BetLevelSizing {
                ip: PotPercentSize { percent_pot: 0.90, include_allin: true },
                oop: PotPercentSize { percent_pot: 1.20, include_allin: true },
            },
        },
        scenarios: None,
    }
}

fn create_spot_config(config: &PreflopConfig, _rfi: &str, _defender: &str) -> Preflop8MaxConfig {
    Preflop8MaxConfig::from_preflop_config(config)
}

#[derive(Debug, Serialize)]
struct SpotResult {
    rfi: String,
    defender: String,
    ci: f64,
    iterations: u64,
    elapsed_secs: f64,
    info_sets: usize,
}

/// Preflop range output for visualization.
#[derive(Debug, Serialize, Deserialize)]
struct PreflopRangeOutput {
    metadata: RangeMetadata,
    spots: Vec<SpotStrategy>,
}

#[derive(Debug, Serialize, Deserialize)]
struct RangeMetadata {
    config_name: String,
    stack_bb: f64,
    total_time_secs: f64,
    timestamp: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct SpotStrategy {
    rfi_position: String,
    defender_position: String,
    /// Strategy for each hand class (0-168)
    /// Maps hand class index to action probabilities
    strategies: HashMap<String, HandStrategy>,
}

#[derive(Debug, Serialize, Deserialize)]
struct HandStrategy {
    hand: String,  // e.g., "AA", "AKs", "72o"
    actions: Vec<String>,
    probabilities: Vec<f64>,
}

fn export_solution(results: &[SpotResult], config: &PreflopConfig, elapsed_secs: f64) {
    let output = PreflopRangeOutput {
        metadata: RangeMetadata {
            config_name: config.name.clone(),
            stack_bb: config.hand_data.stacks.values().next().copied().unwrap_or(50.0),
            total_time_secs: elapsed_secs,
            timestamp: format!("{}", std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs()),
        },
        spots: results.iter().map(|r| SpotStrategy {
            rfi_position: r.rfi.clone(),
            defender_position: r.defender.clone(),
            strategies: HashMap::new(), // Would be populated from solver
        }).collect(),
    };

    let json = serde_json::to_string_pretty(&output).unwrap();
    let filename = "solution_8max.json";
    let mut file = File::create(filename).unwrap();
    file.write_all(json.as_bytes()).unwrap();
    println!("\nSolution exported to: {}", filename);
}
