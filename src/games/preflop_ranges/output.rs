//! Output formatting for preflop ranges.

use std::collections::HashMap;
use std::fs::File;
use std::io::Write;
use serde::{Serialize, Deserialize};

use super::state::{Scenario, ActionType};
use super::{HAND_NAMES, hand_class_to_grid, grid_to_hand_name};

/// Strategy for a single hand
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HandStrategy {
    pub hand: String,
    pub fold: f64,
    pub call: f64,
    pub raise: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allin: Option<f64>,
}

/// Range for a scenario - 13x13 grid
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScenarioRange {
    pub scenario: String,
    pub display_name: String,
    /// Strategies indexed by hand name (e.g., "AA", "AKs")
    pub hands: HashMap<String, HandStrategy>,
    /// 13x13 grid for easy visualization
    pub grid: Vec<Vec<HandStrategy>>,
}

impl ScenarioRange {
    pub fn new(scenario: &Scenario, strategies: &HashMap<u8, Vec<f64>>, actions: &[ActionType]) -> Self {
        let mut hands = HashMap::new();
        let mut grid = vec![vec![HandStrategy {
            hand: String::new(),
            fold: 0.0,
            call: 0.0,
            raise: 0.0,
            allin: None,
        }; 13]; 13];

        // Map action indices
        let fold_idx = actions.iter().position(|a| *a == ActionType::Fold);
        let call_idx = actions.iter().position(|a| *a == ActionType::Call);
        let raise_idx = actions.iter().position(|a| *a == ActionType::Raise);
        let allin_idx = actions.iter().position(|a| *a == ActionType::AllIn);

        for hand_class in 0..169u8 {
            let (row, col) = hand_class_to_grid(hand_class);
            let hand_name = grid_to_hand_name(row, col).to_string();

            let strat = strategies.get(&hand_class).cloned().unwrap_or_else(|| vec![0.0; actions.len()]);

            let fold = fold_idx.map(|i| strat.get(i).copied().unwrap_or(0.0)).unwrap_or(0.0);
            let call = call_idx.map(|i| strat.get(i).copied().unwrap_or(0.0)).unwrap_or(0.0);
            let raise = raise_idx.map(|i| strat.get(i).copied().unwrap_or(0.0)).unwrap_or(0.0);
            let allin = allin_idx.map(|i| strat.get(i).copied().unwrap_or(0.0));

            let hand_strat = HandStrategy {
                hand: hand_name.clone(),
                fold,
                call,
                raise,
                allin,
            };

            hands.insert(hand_name, hand_strat.clone());
            grid[row][col] = hand_strat;
        }

        Self {
            scenario: scenario.name(),
            display_name: scenario.display_name(),
            hands,
            grid,
        }
    }

    /// Get total raise frequency
    pub fn total_raise_freq(&self) -> f64 {
        let total: f64 = self.hands.values().map(|h| h.raise).sum();
        total / 169.0
    }

    /// Get total call frequency
    pub fn total_call_freq(&self) -> f64 {
        let total: f64 = self.hands.values().map(|h| h.call).sum();
        total / 169.0
    }

    /// Print as text grid
    pub fn print_grid(&self) {
        println!("\n=== {} ===", self.display_name);
        println!("Raise: {:.1}% | Call: {:.1}% | Fold: {:.1}%\n",
            self.total_raise_freq() * 100.0,
            self.total_call_freq() * 100.0,
            (1.0 - self.total_raise_freq() - self.total_call_freq()) * 100.0);

        // Header
        print!("     ");
        for col in 0..13 {
            print!("{:>5}", ["A", "K", "Q", "J", "T", "9", "8", "7", "6", "5", "4", "3", "2"][col]);
        }
        println!();

        for row in 0..13 {
            print!("{:>2}   ", ["A", "K", "Q", "J", "T", "9", "8", "7", "6", "5", "4", "3", "2"][row]);
            for col in 0..13 {
                let h = &self.grid[row][col];
                let raise_pct = (h.raise * 100.0).round() as u8;
                if raise_pct >= 90 {
                    print!("\x1b[42m{:>4}\x1b[0m ", raise_pct); // Green
                } else if raise_pct >= 50 {
                    print!("\x1b[43m{:>4}\x1b[0m ", raise_pct); // Yellow
                } else if raise_pct > 0 {
                    print!("\x1b[41m{:>4}\x1b[0m ", raise_pct); // Red
                } else {
                    print!("{:>4} ", "-");
                }
            }
            println!();
        }
    }
}

/// Complete output for all scenarios
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RangeOutput {
    pub metadata: RangeMetadata,
    pub scenarios: Vec<ScenarioRange>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RangeMetadata {
    pub config_name: String,
    pub stack_bb: f64,
    pub iterations: u64,
    pub timestamp: String,
}

impl RangeOutput {
    pub fn new(config_name: &str, stack_bb: f64, iterations: u64) -> Self {
        Self {
            metadata: RangeMetadata {
                config_name: config_name.to_string(),
                stack_bb,
                iterations,
                timestamp: format!("{}", std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs()),
            },
            scenarios: Vec::new(),
        }
    }

    pub fn add_scenario(&mut self, range: ScenarioRange) {
        self.scenarios.push(range);
    }

    pub fn save_json(&self, path: &str) -> std::io::Result<()> {
        let json = serde_json::to_string_pretty(self)?;
        let mut file = File::create(path)?;
        file.write_all(json.as_bytes())?;
        Ok(())
    }

    pub fn print_summary(&self) {
        println!("\n========================================");
        println!("  Preflop Ranges - {}", self.metadata.config_name);
        println!("  Stack: {}bb | Iterations: {}", self.metadata.stack_bb, self.metadata.iterations);
        println!("========================================\n");

        for scenario in &self.scenarios {
            scenario.print_grid();
            println!();
        }
    }
}

/// Generate HTML visualization
pub fn generate_html(output: &RangeOutput) -> String {
    let mut html = String::from(r#"<!DOCTYPE html>
<html>
<head>
    <title>Preflop Ranges</title>
    <style>
        body { font-family: 'Segoe UI', Arial, sans-serif; background: #1a1a2e; color: #eee; padding: 20px; }
        .container { max-width: 1200px; margin: 0 auto; }
        h1 { text-align: center; color: #fff; }
        .metadata { text-align: center; color: #888; margin-bottom: 20px; }
        .scenario { margin-bottom: 40px; background: #252540; padding: 20px; border-radius: 10px; }
        .scenario h2 { margin: 0 0 15px 0; color: #fff; }
        .legend { display: flex; gap: 20px; margin-bottom: 15px; }
        .legend-item { display: flex; align-items: center; gap: 8px; }
        .legend-color { width: 20px; height: 20px; border-radius: 4px; }
        .grid { display: grid; grid-template-columns: repeat(13, 1fr); gap: 2px; }
        .cell { aspect-ratio: 1; display: flex; flex-direction: column; align-items: center; justify-content: center; border-radius: 4px; font-size: 11px; font-weight: bold; }
        .cell .hand { font-size: 13px; }
        .cell .pct { font-size: 10px; opacity: 0.9; }
        .raise { background: linear-gradient(135deg, #2ecc71, #27ae60); color: white; }
        .call { background: linear-gradient(135deg, #3498db, #2980b9); color: white; }
        .fold { background: #444; color: #888; }
        .mixed { color: white; }
    </style>
</head>
<body>
<div class="container">
    <h1>Preflop Ranges</h1>
    <div class="metadata">
"#);

    html.push_str(&format!(
        "{} | Stack: {}bb | {} iterations",
        output.metadata.config_name,
        output.metadata.stack_bb,
        output.metadata.iterations
    ));
    html.push_str("</div>\n");

    for scenario in &output.scenarios {
        html.push_str(&format!(r#"
    <div class="scenario">
        <h2>{}</h2>
        <div class="legend">
            <div class="legend-item"><div class="legend-color" style="background: #2ecc71;"></div>Raise</div>
            <div class="legend-item"><div class="legend-color" style="background: #3498db;"></div>Call</div>
            <div class="legend-item"><div class="legend-color" style="background: #444;"></div>Fold</div>
        </div>
        <div class="grid">
"#, scenario.display_name));

        for row in 0..13 {
            for col in 0..13 {
                let h = &scenario.grid[row][col];
                let raise_pct = (h.raise * 100.0).round() as u32;
                let call_pct = (h.call * 100.0).round() as u32;

                let (class, bg) = if raise_pct >= 80 {
                    ("raise", format!("background: rgba(46, 204, 113, {});", h.raise))
                } else if call_pct >= 80 {
                    ("call", format!("background: rgba(52, 152, 219, {});", h.call))
                } else if raise_pct + call_pct < 20 {
                    ("fold", String::new())
                } else {
                    // Mixed
                    let r = (h.raise * 255.0) as u8;
                    let g = (h.call * 255.0) as u8;
                    ("mixed", format!("background: rgb({}, {}, 100);", 46 + r/2, 100 + g/2))
                };

                let display = if raise_pct > 0 || call_pct > 0 {
                    if raise_pct > 0 && call_pct > 0 {
                        format!("{}%/{}%", raise_pct, call_pct)
                    } else if raise_pct > 0 {
                        format!("{}%", raise_pct)
                    } else {
                        format!("{}%", call_pct)
                    }
                } else {
                    String::from("-")
                };

                html.push_str(&format!(
                    r#"            <div class="cell {}" style="{}"><span class="hand">{}</span><span class="pct">{}</span></div>
"#,
                    class, bg, h.hand, display
                ));
            }
        }

        html.push_str("        </div>\n    </div>\n");
    }

    html.push_str("</div>\n</body>\n</html>");
    html
}
