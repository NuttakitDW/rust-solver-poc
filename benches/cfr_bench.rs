//! Benchmarks for CFR solver.

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use rust_solver_poc::cfr::{CFRConfig, CFRSolver};
use rust_solver_poc::games::kuhn::KuhnPoker;

fn kuhn_iteration_benchmark(c: &mut Criterion) {
    let game = KuhnPoker::new();
    let config = CFRConfig::default().with_seed(42);
    let mut solver = CFRSolver::new(game, config);

    c.bench_function("kuhn_single_iteration", |b| {
        b.iter(|| {
            solver.run_iteration();
            black_box(solver.iteration())
        })
    });
}

fn kuhn_1000_iterations_benchmark(c: &mut Criterion) {
    c.bench_function("kuhn_1000_iterations", |b| {
        b.iter(|| {
            let game = KuhnPoker::new();
            let config = CFRConfig::default().with_seed(42);
            let mut solver = CFRSolver::new(game, config);
            solver.train(black_box(1000))
        })
    });
}

criterion_group!(benches, kuhn_iteration_benchmark, kuhn_1000_iterations_benchmark);
criterion_main!(benches);
