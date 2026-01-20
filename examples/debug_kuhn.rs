//! Debug script for Kuhn Poker CFR

use rust_solver_poc::cfr::{CFRConfig, CFRSolver};
use rust_solver_poc::games::kuhn::KuhnPoker;

fn main() {
    let game = KuhnPoker::new();
    let config = CFRConfig::default().with_seed(42);
    let mut solver = CFRSolver::new(game, config);

    // Run iterations and print progress
    for i in 1..=10 {
        solver.train(10_000);

        let jack = solver.get_average_strategy("0:", 2);
        let queen = solver.get_average_strategy("1:", 2);
        let king = solver.get_average_strategy("2:", 2);

        println!("After {} iterations:", i * 10_000);
        println!("  Jack  (P1 root): Pass={:.3}, Bet={:.3}", jack[0], jack[1]);
        println!("  Queen (P1 root): Pass={:.3}, Bet={:.3}", queen[0], queen[1]);
        println!("  King  (P1 root): Pass={:.3}, Bet={:.3}", king[0], king[1]);

        // Also check P2's strategies facing bet
        let jack_vs_bet = solver.get_average_strategy("0:b", 2);
        let queen_vs_bet = solver.get_average_strategy("1:b", 2);
        let king_vs_bet = solver.get_average_strategy("2:b", 2);

        println!("  P2 Jack facing bet:  Pass(fold)={:.3}, Bet(call)={:.3}", jack_vs_bet[0], jack_vs_bet[1]);
        println!("  P2 Queen facing bet: Pass(fold)={:.3}, Bet(call)={:.3}", queen_vs_bet[0], queen_vs_bet[1]);
        println!("  P2 King facing bet:  Pass(fold)={:.3}, Bet(call)={:.3}", king_vs_bet[0], king_vs_bet[1]);
        println!();
    }

    // Check exploitability
    let exploit = solver.calculate_exploitability(1000);
    println!("Exploitability: {:.4}", exploit);

    // Print info set count
    println!("Total info sets: {}", solver.num_info_sets());

    // Expected Nash equilibrium:
    println!("\nExpected Nash Equilibrium:");
    println!("  P1 Jack:  Pass=0.667, Bet=0.333 (bluff 1/3)");
    println!("  P1 Queen: Pass=1.000, Bet=0.000");
    println!("  P1 King:  Pass=0.000, Bet=1.000");
    println!("  P2 Jack vs bet:  Fold=1.000, Call=0.000");
    println!("  P2 Queen vs bet: Fold=0.667, Call=0.333");
    println!("  P2 King vs bet:  Fold=0.000, Call=1.000");
}
