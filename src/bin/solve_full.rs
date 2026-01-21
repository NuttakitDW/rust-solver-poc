//! SB vs BB full game solver binary.
//!
//! Usage:
//!   cargo run --release --bin solve_full -- [OPTIONS]
//!
//! Options:
//!   --config <FILE>      Configuration JSON file (optional)
//!   --iterations <N>     Number of iterations (default: 100000)
//!   --threads <N>        Number of threads (default: auto)
//!   --output <FILE>      Output file (default: solution.json)
//!   --seed <N>           Random seed (optional)
//!   --fast               Use fast testing configuration

use std::env;
use std::time::Instant;

use rust_solver_poc::cfr::{CFRConfig, CFRSolver};
use rust_solver_poc::games::preflop::{
    game::SBvsBBFullGame,
    postflop_config::FullGameConfig,
    output::SolverOutput,
};

fn main() {
    let args: Vec<String> = env::args().collect();

    // Parse arguments
    let mut config_file: Option<String> = None;
    let mut iterations: u64 = 100_000;
    let mut threads: usize = 0;
    let mut output_file = "solution.json".to_string();
    let mut seed: Option<u64> = None;
    let mut fast_mode = false;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--config" | "-c" => {
                i += 1;
                if i < args.len() {
                    config_file = Some(args[i].clone());
                }
            }
            "--iterations" | "-i" => {
                i += 1;
                if i < args.len() {
                    iterations = args[i].parse().unwrap_or(100_000);
                }
            }
            "--threads" | "-t" => {
                i += 1;
                if i < args.len() {
                    threads = args[i].parse().unwrap_or(0);
                }
            }
            "--output" | "-o" => {
                i += 1;
                if i < args.len() {
                    output_file = args[i].clone();
                }
            }
            "--seed" | "-s" => {
                i += 1;
                if i < args.len() {
                    seed = args[i].parse().ok();
                }
            }
            "--fast" | "-f" => {
                fast_mode = true;
            }
            "--help" | "-h" => {
                print_help();
                return;
            }
            _ => {
                eprintln!("Unknown argument: {}", args[i]);
                print_help();
                return;
            }
        }
        i += 1;
    }

    println!("=================================================");
    println!("  SB vs BB Full Game Solver");
    println!("=================================================");
    println!();

    // Load or create configuration
    let (game, config_name, stack_bb) = if let Some(path) = &config_file {
        println!("Loading configuration from: {}", path);
        match FullGameConfig::from_json_file(path) {
            Ok(config) => {
                let name = config.name.clone();
                let stack = config.stack_bb;
                let game_config = config.to_game_config();
                (SBvsBBFullGame::with_config(game_config), name, stack)
            }
            Err(e) => {
                eprintln!("Error loading config: {}", e);
                return;
            }
        }
    } else if fast_mode {
        println!("Using fast testing configuration");
        (SBvsBBFullGame::fast(), "Fast Test".to_string(), 50.0)
    } else {
        println!("Using default 50bb configuration");
        (SBvsBBFullGame::new(), "Default 50bb".to_string(), 50.0)
    };

    // Setup CFR configuration
    let mut cfr_config = CFRConfig::default();
    if let Some(s) = seed {
        cfr_config = cfr_config.with_seed(s);
    }
    if threads > 0 {
        cfr_config = cfr_config.with_threads(threads);
    }

    println!("Configuration: {}", config_name);
    println!("Stack: {}bb", stack_bb);
    println!("Iterations: {}", iterations);
    println!("Threads: {}", if threads == 0 { "auto".to_string() } else { threads.to_string() });
    if let Some(s) = seed {
        println!("Seed: {}", s);
    }
    println!("Output: {}", output_file);
    println!();

    // Create solver
    let mut solver = CFRSolver::new(game, cfr_config);

    // Training loop with progress
    println!("Starting training...");
    println!();

    let start_time = Instant::now();
    let report_interval = (iterations / 10).max(1000);
    let mut last_report = 0u64;

    for iter in 1..=iterations {
        solver.run_iteration();

        if iter - last_report >= report_interval || iter == iterations {
            let elapsed = start_time.elapsed().as_secs_f64();
            let iters_per_sec = iter as f64 / elapsed;
            let info_sets = solver.num_info_sets();

            println!(
                "Iteration {:>8} | Info sets: {:>8} | Speed: {:>8.0} it/s | Elapsed: {:>6.1}s",
                iter, info_sets, iters_per_sec, elapsed
            );

            last_report = iter;
        }
    }

    let total_time = start_time.elapsed();
    println!();
    println!("Training complete!");
    println!("Total time: {:.2}s", total_time.as_secs_f64());
    println!("Final info sets: {}", solver.num_info_sets());
    println!("Average speed: {:.0} iterations/second",
        iterations as f64 / total_time.as_secs_f64());
    println!();

    // Export results
    println!("Exporting results to {}...", output_file);

    let output = SolverOutput::from_solver(&solver, &config_name, stack_bb);

    match output.save_json(&output_file) {
        Ok(_) => println!("Results saved successfully!"),
        Err(e) => eprintln!("Error saving results: {}", e),
    }

    // Print some sample strategies
    println!();
    println!("=== Sample Strategies ===");
    println!();

    // Get some preflop strategies
    let preflop_strategies = output.preflop_strategies(0); // SB
    for entry in preflop_strategies.iter().take(5) {
        println!("Info state: {}", entry.info_key);
        println!("  Bucket (hand class): {}", entry.bucket);
        for (i, prob) in entry.strategy.iter().enumerate() {
            if *prob > 0.001 {
                let action = entry.actions.get(i).map(|s| s.as_str()).unwrap_or("?");
                println!("  {}: {:.1}%", action, prob * 100.0);
            }
        }
        println!();
    }

    println!("Done!");
}

fn print_help() {
    println!("SB vs BB Full Game Solver");
    println!();
    println!("Usage: solve_full [OPTIONS]");
    println!();
    println!("Options:");
    println!("  -c, --config <FILE>      Configuration JSON file");
    println!("  -i, --iterations <N>     Number of iterations (default: 100000)");
    println!("  -t, --threads <N>        Number of threads (default: auto)");
    println!("  -o, --output <FILE>      Output file (default: solution.json)");
    println!("  -s, --seed <N>           Random seed");
    println!("  -f, --fast               Use fast testing configuration");
    println!("  -h, --help               Show this help");
    println!();
    println!("Examples:");
    println!("  solve_full --fast --iterations 10000");
    println!("  solve_full --config configs/sb_vs_bb_50bb.json");
    println!("  solve_full -i 500000 -t 8 -o my_solution.json");
}
