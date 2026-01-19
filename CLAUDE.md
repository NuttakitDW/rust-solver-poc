# rust-solver-poc

## Project Overview
Stage: 0
Setup POC for CFR solver to create preflop range for 50bb stack.

## Tech Stack
- **Language**: Rust
- **Build Tool**: Cargo

## Project Structure
```
src/
├── main.rs          # Entry point
├── lib.rs           # Library exports
└── ...
```

## Development Commands
```bash
# Build
cargo build

# Run
cargo run

# Test
cargo test

# Format
cargo fmt

# Lint
cargo clippy
```

## Architecture
<!-- Describe the solver architecture, algorithms used, etc. -->

## Configuration
<!-- Environment variables, config files, etc. -->

## Testing
<!-- How to run tests, test coverage requirements -->

## Notes
- use solver.md as refferrence when implement a solver.
- cold call is when players call 3bet without involve with previous action for example UTG raise UTG1 3bet and LJ call we call LJ call = "Cold Call".
- flat call flat call is action that call to match raise from RFI.