//! Preflop Range Solver
//!
//! Solves preflop ranges for all positions and scenarios.
//! Outputs human-readable JSON with hand names (AA, AKs, etc.)

use std::time::Instant;
use std::fs::File;
use std::io::Write;

use rust_solver_poc::games::preflop_ranges::{
    Position, Scenario, ActionType,
    PreflopRangeConfig, PreflopRangeGame, solve_scenario,
    RangeOutput, ScenarioRange, generate_html,
};

fn main() {
    println!("=== Preflop Range Solver ===\n");

    let start = Instant::now();
    let config = PreflopRangeConfig::default();
    let iterations = 10000u64;

    println!("Stack: {}bb | Iterations per scenario: {}", config.stack_bb, iterations);
    println!();

    let mut output = RangeOutput::new("HRC 8-max 50bb", config.stack_bb, iterations);

    // Define scenarios to solve
    let scenarios = vec![
        // RFI for each position
        Scenario::RFI { position: Position::UTG },
        Scenario::RFI { position: Position::EP },
        Scenario::RFI { position: Position::MP },
        Scenario::RFI { position: Position::HJ },
        Scenario::RFI { position: Position::CO },
        Scenario::RFI { position: Position::BU },
        Scenario::RFI { position: Position::SB },

        // BB defense vs each position
        Scenario::VsRFI { hero: Position::BB, villain: Position::UTG },
        Scenario::VsRFI { hero: Position::BB, villain: Position::EP },
        Scenario::VsRFI { hero: Position::BB, villain: Position::MP },
        Scenario::VsRFI { hero: Position::BB, villain: Position::HJ },
        Scenario::VsRFI { hero: Position::BB, villain: Position::CO },
        Scenario::VsRFI { hero: Position::BB, villain: Position::BU },
        Scenario::VsRFI { hero: Position::BB, villain: Position::SB },

        // SB defense vs BU
        Scenario::VsRFI { hero: Position::SB, villain: Position::BU },

        // 3bet scenarios
        Scenario::ThreeBet { hero: Position::BB, villain: Position::BU },
        Scenario::ThreeBet { hero: Position::SB, villain: Position::BU },

        // Facing 3bet
        Scenario::Vs3Bet { hero: Position::BU, villain: Position::BB },
        Scenario::Vs3Bet { hero: Position::BU, villain: Position::SB },
    ];

    println!("Solving {} scenarios...\n", scenarios.len());

    for (i, scenario) in scenarios.iter().enumerate() {
        let scenario_start = Instant::now();

        print!("[{}/{}] {} ... ",
            i + 1, scenarios.len(), scenario.display_name());

        // Get actions for this scenario
        let game = PreflopRangeGame::new(scenario.clone(), config.clone());
        let actions: Vec<ActionType> = match scenario {
            Scenario::RFI { .. } => vec![ActionType::Fold, ActionType::Raise],
            Scenario::VsRFI { .. } => vec![ActionType::Fold, ActionType::Call, ActionType::Raise],
            Scenario::Vs3Bet { .. } => vec![ActionType::Fold, ActionType::Call, ActionType::Raise],
            Scenario::Vs4Bet { .. } => vec![ActionType::Fold, ActionType::Call, ActionType::AllIn],
            _ => vec![ActionType::Fold, ActionType::Raise],
        };

        // Solve
        let strategies = solve_scenario(scenario.clone(), &config, iterations);

        // Create range output
        let range = ScenarioRange::new(scenario, &strategies, &actions);

        println!("done ({:.2}s) - Raise: {:.1}%",
            scenario_start.elapsed().as_secs_f64(),
            range.total_raise_freq() * 100.0);

        output.add_scenario(range);
    }

    let elapsed = start.elapsed();

    println!("\n=== Complete ===");
    println!("Total time: {:.2}s", elapsed.as_secs_f64());

    // Save JSON
    let json_path = "preflop_ranges.json";
    match output.save_json(json_path) {
        Ok(_) => println!("Saved JSON: {}", json_path),
        Err(e) => eprintln!("Error saving JSON: {}", e),
    }

    // Save HTML
    let html_path = "preflop_ranges.html";
    let html = generate_html(&output);
    match File::create(html_path).and_then(|mut f| f.write_all(html.as_bytes())) {
        Ok(_) => println!("Saved HTML: {}", html_path),
        Err(e) => eprintln!("Error saving HTML: {}", e),
    }

    // Print summary
    println!("\n");
    output.print_summary();
}
