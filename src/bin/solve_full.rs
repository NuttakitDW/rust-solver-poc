//! SB vs BB full game solver binary.
//!
//! Usage:
//!   cargo run --release --bin solve_full -- [OPTIONS]
//!
//! Options:
//!   --config <FILE>      Configuration JSON file (optional)
//!   --ci <VALUE>         Target CI (Convergence Indicator) - stops when reached
//!   --iterations <N>     Max iterations (default: unlimited when using --ci)
//!   --threads <N>        Number of threads (default: auto)
//!   --output <FILE>      Output file (default: solution.json)
//!   --seed <N>           Random seed (optional)
//!   --fast               Use fast testing configuration

use std::env;
use std::time::Instant;

use rust_solver_poc::cfr::{CFRConfig, CFRSolver, ConvergenceStats};
use rust_solver_poc::games::preflop::{
    game::SBvsBBFullGame,
    postflop_config::FullGameConfig,
    output::SolverOutput,
};

fn main() {
    let args: Vec<String> = env::args().collect();

    // Parse arguments
    let mut config_file: Option<String> = None;
    let mut ci_target: Option<f64> = None;
    let mut max_iterations: u64 = 0; // 0 = no limit when using CI
    let mut iterations: Option<u64> = None;
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
            "--ci" => {
                i += 1;
                if i < args.len() {
                    ci_target = args[i].parse().ok();
                }
            }
            "--iterations" | "-i" => {
                i += 1;
                if i < args.len() {
                    iterations = args[i].parse().ok();
                }
            }
            "--max-iterations" => {
                i += 1;
                if i < args.len() {
                    max_iterations = args[i].parse().unwrap_or(0);
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

    // Determine stopping mode
    let use_ci_mode = ci_target.is_some();

    if let Some(ci) = ci_target {
        println!("Target CI: {} (Convergence Indicator)", ci);
        println!("  CI < 10: bare minimum");
        println!("  CI ~ 1 : fully converged");
        if max_iterations > 0 {
            println!("Max iterations: {}", max_iterations);
        } else {
            println!("Max iterations: unlimited");
        }
    } else {
        let iter_count = iterations.unwrap_or(100_000);
        println!("Iterations: {}", iter_count);
    }

    println!("Threads: {}", if threads == 0 { "auto".to_string() } else { threads.to_string() });
    if let Some(s) = seed {
        println!("Seed: {}", s);
    }
    println!("Output: {}", output_file);
    println!();

    // Create solver
    let mut solver = CFRSolver::new(game, cfr_config);

    // Training
    println!("Starting training...");
    println!();

    let start_time = Instant::now();

    if use_ci_mode {
        // CI-based convergence mode
        let ci = ci_target.unwrap();
        let ci_check_interval = 1000u64; // Check CI every 1000 iterations

        let result = solver.train_until_converged(
            ci,
            ci_check_interval,
            max_iterations,
            Some(|stats: &ConvergenceStats| {
                println!(
                    "Iteration {:>8} | CI: {:>6.2} | Info sets: {:>8} | Speed: {:>6.0} it/s | Elapsed: {:>6.1}s",
                    stats.iteration,
                    stats.ci,
                    stats.info_sets,
                    stats.iterations_per_second,
                    stats.elapsed_seconds
                );
            }),
        );

        println!();
        if result.converged {
            println!("Converged! Final CI: {:.2} (target: {:.2})", result.final_ci, ci);
        } else {
            println!("Stopped at max iterations. Final CI: {:.2} (target: {:.2})", result.final_ci, ci);
        }
        println!("Total iterations: {}", result.iterations);
        println!("Total time: {:.2}s", result.elapsed_seconds);

    } else {
        // Iteration-based mode
        let iter_count = iterations.unwrap_or(100_000);
        let report_interval = (iter_count / 10).max(1000);
        let mut last_report = 0u64;

        // Take initial snapshot for CI reporting
        let mut snapshot = solver.snapshot_strategies();
        let mut last_ci = f64::INFINITY;

        for iter in 1..=iter_count {
            solver.run_iteration();

            if iter - last_report >= report_interval || iter == iter_count {
                let elapsed = start_time.elapsed().as_secs_f64();
                let iters_per_sec = iter as f64 / elapsed;
                let info_sets = solver.num_info_sets();

                // Calculate CI if we have enough data
                if iter > 1000 {
                    last_ci = solver.calculate_ci(&snapshot);
                    snapshot = solver.snapshot_strategies();
                }

                println!(
                    "Iteration {:>8} | CI: {:>6.2} | Info sets: {:>8} | Speed: {:>6.0} it/s | Elapsed: {:>6.1}s",
                    iter,
                    if last_ci.is_infinite() { 0.0 } else { last_ci },
                    info_sets,
                    iters_per_sec,
                    elapsed
                );

                last_report = iter;
            }
        }

        let total_time = start_time.elapsed();
        println!();
        println!("Training complete!");
        println!("Total time: {:.2}s", total_time.as_secs_f64());
        println!("Final CI: {:.2}", if last_ci.is_infinite() { 0.0 } else { last_ci });
    }

    println!("Final info sets: {}", solver.num_info_sets());
    println!("Average speed: {:.0} iterations/second",
        solver.iteration() as f64 / start_time.elapsed().as_secs_f64());
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
    println!("Stopping Criteria (choose one):");
    println!("  --ci <VALUE>             Target CI (Convergence Indicator) - recommended");
    println!("                           CI < 10: bare minimum, CI ~ 1: fully converged");
    println!("  -i, --iterations <N>     Fixed number of iterations");
    println!();
    println!("Options:");
    println!("  -c, --config <FILE>      Configuration JSON file");
    println!("  --max-iterations <N>     Max iterations when using --ci (default: unlimited)");
    println!("  -t, --threads <N>        Number of threads (default: auto)");
    println!("  -o, --output <FILE>      Output file (default: solution.json)");
    println!("  -s, --seed <N>           Random seed");
    println!("  -f, --fast               Use fast testing configuration");
    println!("  -h, --help               Show this help");
    println!();
    println!("Examples:");
    println!("  # Solve until CI reaches 10 (bare minimum convergence)");
    println!("  solve_full --ci 10 --fast");
    println!();
    println!("  # Solve until CI reaches 1 (fully converged) with max 1M iterations");
    println!("  solve_full --ci 1 --max-iterations 1000000");
    println!();
    println!("  # Fixed 100k iterations (shows CI during training)");
    println!("  solve_full --iterations 100000 --fast");
    println!();
    println!("  # Use custom config");
    println!("  solve_full --ci 5 --config configs/sb_vs_bb_50bb_full.json");
}
