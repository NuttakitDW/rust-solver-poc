//! Preflop range game state.

use std::fmt;

/// Position in poker (8-max)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Position {
    UTG, EP, MP, HJ, CO, BU, SB, BB,
}

impl Position {
    pub fn name(&self) -> &'static str {
        match self {
            Position::UTG => "UTG",
            Position::EP => "EP",
            Position::MP => "MP",
            Position::HJ => "HJ",
            Position::CO => "CO",
            Position::BU => "BU",
            Position::SB => "SB",
            Position::BB => "BB",
        }
    }

    pub fn all() -> &'static [Position] {
        &[Position::UTG, Position::EP, Position::MP, Position::HJ,
          Position::CO, Position::BU, Position::SB, Position::BB]
    }

    pub fn index(&self) -> usize {
        match self {
            Position::UTG => 0,
            Position::EP => 1,
            Position::MP => 2,
            Position::HJ => 3,
            Position::CO => 4,
            Position::BU => 5,
            Position::SB => 6,
            Position::BB => 7,
        }
    }
}

impl fmt::Display for Position {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name())
    }
}

/// Preflop scenario type
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Scenario {
    /// RFI (Raise First In) - opening from a position
    RFI { position: Position },
    /// Facing RFI - responding to an open
    VsRFI { hero: Position, villain: Position },
    /// Facing 3bet
    Vs3Bet { hero: Position, villain: Position },
    /// 3bet scenario (hero 3bets)
    ThreeBet { hero: Position, villain: Position },
    /// Facing 4bet
    Vs4Bet { hero: Position, villain: Position },
    /// 4bet scenario
    FourBet { hero: Position, villain: Position },
    /// Facing 5bet (usually just call or fold)
    Vs5Bet { hero: Position, villain: Position },
}

impl Scenario {
    pub fn name(&self) -> String {
        match self {
            Scenario::RFI { position } => format!("{}_RFI", position.name()),
            Scenario::VsRFI { hero, villain } => format!("{}_vs_{}_RFI", hero.name(), villain.name()),
            Scenario::ThreeBet { hero, villain } => format!("{}_3bet_vs_{}", hero.name(), villain.name()),
            Scenario::Vs3Bet { hero, villain } => format!("{}_vs_{}_3bet", hero.name(), villain.name()),
            Scenario::FourBet { hero, villain } => format!("{}_4bet_vs_{}", hero.name(), villain.name()),
            Scenario::Vs4Bet { hero, villain } => format!("{}_vs_{}_4bet", hero.name(), villain.name()),
            Scenario::Vs5Bet { hero, villain } => format!("{}_vs_{}_5bet", hero.name(), villain.name()),
        }
    }

    pub fn display_name(&self) -> String {
        match self {
            Scenario::RFI { position } => format!("{} Open (RFI)", position.name()),
            Scenario::VsRFI { hero, villain } => format!("{} vs {} Open", hero.name(), villain.name()),
            Scenario::ThreeBet { hero, villain } => format!("{} 3-Bet vs {}", hero.name(), villain.name()),
            Scenario::Vs3Bet { hero, villain } => format!("{} vs {} 3-Bet", hero.name(), villain.name()),
            Scenario::FourBet { hero, villain } => format!("{} 4-Bet vs {}", hero.name(), villain.name()),
            Scenario::Vs4Bet { hero, villain } => format!("{} vs {} 4-Bet", hero.name(), villain.name()),
            Scenario::Vs5Bet { hero, villain } => format!("{} vs {} 5-Bet", hero.name(), villain.name()),
        }
    }
}

/// Action type for preflop
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ActionType {
    Fold,
    Call,   // or Limp for RFI from SB
    Raise,  // Open, 3bet, 4bet, 5bet depending on context
    AllIn,
}

impl ActionType {
    pub fn name(&self) -> &'static str {
        match self {
            ActionType::Fold => "Fold",
            ActionType::Call => "Call",
            ActionType::Raise => "Raise",
            ActionType::AllIn => "All-In",
        }
    }
}

/// Game state for preflop range solving
#[derive(Clone, Debug)]
pub struct PreflopRangeState {
    /// Current scenario being solved
    pub scenario: Scenario,
    /// Hand class (0-168)
    pub hand_class: u8,
    /// Whether we've made a decision
    pub decided: bool,
    /// The action taken (if decided)
    pub action: Option<ActionType>,
}

impl PreflopRangeState {
    pub fn new(scenario: Scenario, hand_class: u8) -> Self {
        Self {
            scenario,
            hand_class,
            decided: false,
            action: None,
        }
    }

    pub fn with_action(mut self, action: ActionType) -> Self {
        self.decided = true;
        self.action = Some(action);
        self
    }
}
