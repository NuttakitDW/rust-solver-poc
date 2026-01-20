//! Game trait definition for CFR solver.
//!
//! Any game that implements the `Game` trait can be solved using CFR.
//! This provides a clean abstraction between the algorithm and specific games.

use std::fmt::Debug;
use std::hash::Hash;

/// Trait for actions that can be taken in a game.
///
/// Actions must be cloneable, comparable, and hashable for storage in maps.
pub trait Action: Clone + Eq + Hash + Debug + Send + Sync {
    /// Convert action to a string representation for display/storage.
    fn to_string(&self) -> String;
}

/// Trait for information states (what a player knows at a decision point).
///
/// An information state represents all the information available to a player
/// when making a decision. Two game states that look identical to a player
/// (same cards, same action history) should produce the same information state.
pub trait InfoState: Clone + Eq + Hash + Debug + Send + Sync {
    /// Generate a unique string key for this information state.
    /// This key is used for storing regrets and strategies.
    fn key(&self) -> String;
}

/// Trait for game states.
///
/// A game state contains all information about the current state of the game,
/// including private information that players may not see.
pub trait GameState: Clone + Debug + Send + Sync {}

/// The main Game trait that defines the interface for any game.
///
/// Implement this trait to use the CFR solver with your game.
///
/// # Type Parameters
/// - `S`: The game state type
/// - `A`: The action type
/// - `I`: The information state type
///
/// # Example
/// ```ignore
/// struct MyGame;
///
/// impl Game for MyGame {
///     type State = MyGameState;
///     type Action = MyAction;
///     type InfoState = MyInfoState;
///
///     // ... implement required methods
/// }
/// ```
pub trait Game: Clone + Send + Sync {
    /// The type representing a complete game state.
    type State: GameState;

    /// The type representing an action a player can take.
    type Action: Action;

    /// The type representing what a player knows at a decision point.
    type InfoState: InfoState;

    /// Create the initial game state.
    ///
    /// This is called at the start of each CFR traversal to get a fresh game.
    fn initial_state(&self) -> Self::State;

    /// Check if the given state is terminal (game over).
    ///
    /// Terminal states have no more actions available and payoffs can be computed.
    fn is_terminal(&self, state: &Self::State) -> bool;

    /// Get the payoff for a player at a terminal state.
    ///
    /// # Arguments
    /// * `state` - A terminal game state
    /// * `player` - The player index (0-indexed)
    ///
    /// # Returns
    /// The payoff (utility) for the specified player. Positive values indicate
    /// a win, negative values indicate a loss.
    ///
    /// # Panics
    /// May panic if called on a non-terminal state.
    fn get_payoff(&self, state: &Self::State, player: usize) -> f64;

    /// Get the index of the player who should act at the current state.
    ///
    /// # Returns
    /// - `Some(player_index)` if a player should act
    /// - `None` if the state is terminal or a chance node
    fn current_player(&self, state: &Self::State) -> Option<usize>;

    /// Get the total number of players in the game.
    fn num_players(&self) -> usize;

    /// Get the list of available actions at the current state.
    ///
    /// # Returns
    /// A vector of actions the current player can take.
    /// Returns empty vector if state is terminal.
    fn available_actions(&self, state: &Self::State) -> Vec<Self::Action>;

    /// Apply an action to a state and return the resulting new state.
    ///
    /// This should not modify the input state (immutable transition).
    ///
    /// # Arguments
    /// * `state` - The current game state
    /// * `action` - The action to apply
    ///
    /// # Returns
    /// The new game state after applying the action.
    fn apply_action(&self, state: &Self::State, action: &Self::Action) -> Self::State;

    /// Get the information state for the current player.
    ///
    /// The information state captures everything the current player knows,
    /// which typically includes their private cards and the public action history,
    /// but not other players' private information.
    ///
    /// # Arguments
    /// * `state` - The current game state
    ///
    /// # Returns
    /// The information state for the player who is currently acting.
    fn info_state(&self, state: &Self::State) -> Self::InfoState;

    /// Check if the current state is a chance node.
    ///
    /// Chance nodes represent random events like dealing cards.
    /// Override this if your game has chance nodes.
    ///
    /// # Returns
    /// `true` if the state is a chance node, `false` otherwise.
    fn is_chance(&self, _state: &Self::State) -> bool {
        false
    }

    /// Sample an outcome from a chance node.
    ///
    /// This is called when the game reaches a chance node to randomly
    /// determine the outcome (e.g., which cards are dealt).
    ///
    /// # Arguments
    /// * `state` - The current chance state
    /// * `rng` - A random number generator
    ///
    /// # Returns
    /// The new state after sampling the chance outcome.
    fn sample_chance<R: rand::Rng>(&self, state: &Self::State, _rng: &mut R) -> Self::State {
        // Default implementation: just return the state unchanged
        // Override this for games with chance nodes
        state.clone()
    }

    /// Get a human-readable name for an action.
    ///
    /// Used for debugging and visualization.
    fn action_name(&self, action: &Self::Action) -> String {
        action.to_string()
    }

    /// Get a human-readable description of a state.
    ///
    /// Used for debugging and visualization.
    fn state_description(&self, state: &Self::State) -> String {
        format!("{:?}", state)
    }
}

/// Macro to simplify implementing the Action trait for simple enums.
#[macro_export]
macro_rules! impl_action {
    ($type:ty) => {
        impl $crate::cfr::game::Action for $type {
            fn to_string(&self) -> String {
                format!("{:?}", self)
            }
        }
    };
}

/// Macro to simplify implementing the GameState trait.
#[macro_export]
macro_rules! impl_game_state {
    ($type:ty) => {
        impl $crate::cfr::game::GameState for $type {}
    };
}
