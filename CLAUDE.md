# rust-solver-poc

## Project Overview

**Stage**: 0 - Proof of Concept
**Goal**: CFR solver to create preflop ranges for 8-max cash games at 50bb stack depth
**Reference**: HRC (Holdem Resources Calculator) - target <2 minute solve time for all positions
**Algorithm**: Counterfactual Regret Minimization (CFR) with CFR+ and Linear CFR optimizations

## Tech Stack

- **Language**: Rust (2021 edition)
- **Build Tool**: Cargo
- **Key Dependencies**:
  - `rand` (0.8) - RNG for card dealing and MCCFR sampling
  - `rayon` (1.10) - Parallel computation
  - `rustc-hash` (2.0) - Fast HashMaps for regret/strategy storage
  - `serde/serde_json` (1.0) - Config loading and solution export
  - `indicatif` (0.17) - Progress bars

## Project Structure

```
rust-solver-poc/
├── src/
│   ├── lib.rs                    # Library exports
│   ├── cfr/                      # Core CFR algorithm (GENERIC)
│   │   ├── mod.rs
│   │   ├── game.rs              # Game trait (implement for any game)
│   │   ├── solver.rs            # CFRSolver<G> - main algorithm
│   │   ├── config.rs            # CFRConfig, CFRStats
│   │   └── storage.rs           # Thread-safe regret/strategy storage
│   ├── games/                    # Game implementations
│   │   ├── mod.rs
│   │   ├── kuhn/                # Kuhn Poker (validation game)
│   │   │   └── mod.rs           # 502 lines - known Nash for testing
│   │   ├── preflop/             # Full postflop solver (COMPLEX)
│   │   │   ├── mod.rs
│   │   │   ├── game.rs          # SBvsBBFullGame
│   │   │   ├── state.rs         # Game state
│   │   │   ├── action.rs        # Actions + betting
│   │   │   ├── hand.rs          # Hand representation
│   │   │   ├── hand_eval.rs     # Hand strength evaluation
│   │   │   ├── card.rs          # Card/suit handling
│   │   │   ├── abstraction.rs   # Postflop card bucketing
│   │   │   ├── config.rs        # Game configuration
│   │   │   ├── postflop_config.rs
│   │   │   ├── betting.rs       # Bet sizing logic
│   │   │   ├── info_state.rs    # Information state
│   │   │   └── output.rs        # Solution formatting
│   │   ├── preflop_8max/        # 8-max equity-based (FAST)
│   │   │   ├── mod.rs
│   │   │   ├── state.rs         # 8 positions, actions, hands
│   │   │   ├── action.rs        # Action encoding
│   │   │   ├── game.rs          # Preflop8MaxGame + Config
│   │   │   └── equity.rs        # Equity realization calculator
│   │   └── preflop_ranges/      # Range output (HUMAN-READABLE)
│   │       ├── mod.rs           # Hand names (169 hands: AA-22)
│   │       ├── state.rs         # Scenario-specific state
│   │       ├── game.rs          # PreflopRangeGame
│   │       └── output.rs        # HTML visualization
│   └── bin/                      # Binary entry points
│       ├── solve_full.rs        # SB vs BB full postflop
│       ├── solve_8max.rs        # 8-max parallel solver
│       └── solve_ranges.rs      # Human-readable ranges
├── configs/                      # Configuration files
│   ├── hrc_8max_50bb.json       # HRC-matching settings
│   ├── preflop_8max_50bb.json
│   ├── sb_vs_bb_50bb_full.json
│   └── sb_vs_bb_50bb_ante.json
├── docs/
│   └── hrc_reference_settings.md # HRC settings reference
├── benches/
│   └── cfr_bench.rs             # Criterion benchmarks
├── examples/
│   └── debug_kuhn.rs            # Kuhn debug example
├── solver.md                     # CFR theory reference
├── visualize.py                  # HTML range visualization
├── preflop_ranges.json          # Output: solved ranges
├── solution.json                # Output: full solution
└── solution_8max.json           # Output: 8-max solution
```

## Development Commands

```bash
# Build (debug)
cargo build

# Build (release - REQUIRED for performance)
cargo build --release

# Run binaries
cargo run --release --bin solve_full
cargo run --release --bin solve_8max
cargo run --release --bin solve_ranges

# Run with options
cargo run --release --bin solve_full -- --config configs/hrc_8max_50bb.json --ci 10 --threads 8

# Test (Kuhn poker convergence validation)
cargo test

# Format
cargo fmt

# Lint
cargo clippy

# Benchmark
cargo bench

# Visualize ranges (Python)
python3 visualize.py solution.json range.html
```

## Architecture

### Core CFR Module (`src/cfr/`)

```
┌─────────────────────────────────────────────────────────────┐
│                   CFR Solver (Generic)                      │
│  - Regret accumulation     - Strategy computation           │
│  - MCCFR traversal         - Exploitability calculation     │
│  - Parallel execution      - Convergence tracking           │
└──────────────────────┬──────────────────────────────────────┘
                       │ implements Game trait
                       ▼
        ┌──────────────┼──────────────┐
        │              │              │
        ▼              ▼              ▼
    ┌────────┐  ┌─────────────┐  ┌──────────┐
    │ Kuhn   │  │ Preflop8Max │  │ Preflop  │
    │ Poker  │  │   (8-max)   │  │ Full     │
    │ (test) │  │             │  │ (SBvsBB) │
    └────────┘  ├─────────────┤  └──────────┘
                │ RangeOutput │
                │ (human UI)  │
                └─────────────┘
```

### Game Trait (`src/cfr/game.rs`)

Any game must implement:
```rust
trait Game {
    type State: Clone;      // Complete game state
    type Action: Clone;     // Player actions
    type InfoState: Hash;   // What player knows

    fn initial_state() -> State;
    fn is_terminal(state: &State) -> bool;
    fn get_payoff(state: &State, player: usize) -> f64;
    fn current_player(state: &State) -> usize;
    fn available_actions(state: &State) -> Vec<Action>;
    fn apply_action(state: &State, action: Action) -> State;
    fn info_state(state: &State) -> InfoState;
    fn is_chance(state: &State) -> bool;
    fn sample_chance(state: &State, rng: &mut impl Rng) -> State;
}
```

### Convergence Indicator (CI)

- **Definition**: Measures strategy stability between iterations
- **Target**: CI < 10.0 for solid play (HRC default)
- **Lower = better** (closer to Nash equilibrium)
- **Calculation**: Average strategy change weighted by reach probability

## Configuration Reference (HRC 8-max 50bb)

### Blinds & Antes
| Setting | Value |
|---------|-------|
| SB | 0.5 |
| BB | 1.0 |
| Ante | 0.12 per player |

### Opening Sizes (bb)
| Position | Base | Per Caller |
|----------|------|------------|
| General | 2.3 | +1.0 |
| BU | 2.3 | +1.0 |
| SB | 3.5 | +1.0 |
| BB vs SB | 3.0 | +0.0 |

### 3-Bet Sizes (multiplier of open)
| Context | Base | Per Caller |
|---------|------|------------|
| IP | 2.5x | +1.0x |
| BB vs SB | 2.5x | +0.0x |
| BB vs others | 3.3x | +1.0x |
| SB vs others | 3.3x | +1.0x |

### 4-Bet/5-Bet
- IP: 90% pot, include all-in
- OOP: 120% pot, include all-in

### Action Restrictions (IMPORTANT)
| Setting | Value | Notes |
|---------|-------|-------|
| Cold calls | NO | Simplifies tree significantly |
| Limps | NO | Pure raise/fold preflop |
| Closing flats | YES | Can flat when closing action |
| SB complete | YES | SB can complete vs limps |
| All-in threshold | 40% | Below this % of stack = all-in |
| All-in SPR | 7.0 | Add all-in below this SPR |

### Flats Per Raise Level
```
[limps=0, opens=1, 3bet=1, 4bet=1, 5bet=0]
```

## Key Algorithms

### CFR (Counterfactual Regret Minimization)
1. Initialize uniform random strategies
2. For each iteration:
   - Traverse game tree
   - Calculate counterfactual values
   - Update regrets: `regret[a] += cfv[a] - ev_strategy`
   - Update cumulative strategy
3. Compute average strategy → Nash equilibrium

### CFR+ Optimization
- Floor negative regrets to 0
- Faster convergence (2-10x fewer iterations)

### Linear CFR
- Weight iterations linearly (iteration n weighted by n)
- Recent iterations count more

### Regret Matching
```
strategy[a] = max(0, regret[a]) / sum(max(0, regret[all_actions]))
```

## Scenarios Solved by solve_ranges

**RFI (7 scenarios)**:
- UTG, EP, MP, HJ, CO, BU, SB opening ranges

**VsRFI (7 scenarios)**:
- BB defense vs each position
- SB vs BU flat/3bet

**3-Bet (2 scenarios)**:
- BB 3bet vs BU
- SB 3bet vs BU

**Vs3-Bet (2 scenarios)**:
- BU vs BB 3bet
- BU vs SB 3bet

## Output Format

### Solution JSON Structure
```json
{
  "metadata": {
    "config_name": "HRC 8-max 50bb",
    "stack_bb": 50.0,
    "iterations": 10000,
    "convergence_indicator": 9.87
  },
  "strategies": {
    "info_key": {
      "actions": ["Fold", "Call", "Raise 2.3bb"],
      "strategy": [0.1, 0.3, 0.6],
      "history": "action_sequence"
    }
  }
}
```

### Hand Buckets (169 total)
- **0-12**: Pairs (22=0, AA=12)
- **13-90**: Suited hands
- **91-168**: Offsuit hands

## Poker Terminology

| Term | Definition |
|------|------------|
| **RFI** | Raise First In - opening the pot with a raise |
| **Cold Call** | Calling a 3bet without previous action (e.g., UTG opens, MP 3bets, HJ calls = cold call) |
| **Flat Call** | Calling to match a raise from RFI |
| **IP** | In Position - acting last on subsequent streets |
| **OOP** | Out Of Position - acting first |
| **SPR** | Stack-to-Pot Ratio |
| **CI** | Convergence Indicator |
| **Nash Equilibrium** | Strategy where no player benefits from unilaterally changing |

## Known Issues / TODOs

- [ ] Full postflop solver is slow (needs card abstraction optimization)
- [ ] 8-max solver uses equity approximation (not full tree)
- [ ] HTML visualization could be more interactive
- [ ] Missing ICM support for tournaments

## Testing & Validation

### Kuhn Poker (Ground Truth)
- 3 cards: J < Q < K
- Known Nash equilibrium:
  - P1 Jack: Bet 33%, Pass 67%
  - P1 Queen: Always Pass
  - P1 King: Always Bet
  - Expected Value: -1/18 ≈ -0.0556

Run tests to validate CFR implementation:
```bash
cargo test kuhn
```

## Performance Notes

- **Kuhn**: ~10,000 iterations for convergence
- **8-max preflop**: <2 minutes target (parallelized)
- **Release build REQUIRED**: 10-50x faster than debug
- **Memory**: ~100MB typical

## References

- `solver.md` - CFR theory from GTO Wizard
- `docs/hrc_reference_settings.md` - HRC configuration reference
- `configs/hrc_8max_50bb.json` - Production config
