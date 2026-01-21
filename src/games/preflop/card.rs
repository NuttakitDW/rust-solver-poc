//! Card representation for poker.
//!
//! This module provides fundamental card types used throughout the poker solver:
//! - `Card`: A single playing card with rank and suit
//! - `HoleCards`: A player's two private cards
//! - `Board`: Community cards (0-5 cards)
//! - `Deck`: A deck of 52 cards with dealing functionality

use rand::seq::SliceRandom;
use rand::Rng;
use std::fmt;

/// Rank of a card (0-12: 2-A).
pub const RANK_2: u8 = 0;
pub const RANK_3: u8 = 1;
pub const RANK_4: u8 = 2;
pub const RANK_5: u8 = 3;
pub const RANK_6: u8 = 4;
pub const RANK_7: u8 = 5;
pub const RANK_8: u8 = 6;
pub const RANK_9: u8 = 7;
pub const RANK_T: u8 = 8;
pub const RANK_J: u8 = 9;
pub const RANK_Q: u8 = 10;
pub const RANK_K: u8 = 11;
pub const RANK_A: u8 = 12;

/// Suit of a card (0-3).
pub const SUIT_CLUBS: u8 = 0;
pub const SUIT_DIAMONDS: u8 = 1;
pub const SUIT_HEARTS: u8 = 2;
pub const SUIT_SPADES: u8 = 3;

/// Rank characters for display.
const RANK_CHARS: [char; 13] = ['2', '3', '4', '5', '6', '7', '8', '9', 'T', 'J', 'Q', 'K', 'A'];

/// Suit characters for display.
const SUIT_CHARS: [char; 4] = ['c', 'd', 'h', 's'];

/// A single playing card.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Card {
    /// Card index 0-51: rank * 4 + suit
    id: u8,
}

impl Card {
    /// Create a new card from rank (0-12) and suit (0-3).
    #[inline]
    pub fn new(rank: u8, suit: u8) -> Self {
        debug_assert!(rank < 13, "rank must be 0-12");
        debug_assert!(suit < 4, "suit must be 0-3");
        Self { id: rank * 4 + suit }
    }

    /// Create a card from its ID (0-51).
    #[inline]
    pub fn from_id(id: u8) -> Self {
        debug_assert!(id < 52, "card id must be 0-51");
        Self { id }
    }

    /// Parse a card from string like "As", "Kh", "2c".
    pub fn from_str(s: &str) -> Option<Self> {
        let chars: Vec<char> = s.chars().collect();
        if chars.len() != 2 {
            return None;
        }

        let rank = RANK_CHARS.iter().position(|&c| c == chars[0].to_ascii_uppercase())?;
        let suit = SUIT_CHARS.iter().position(|&c| c == chars[1].to_ascii_lowercase())?;

        Some(Self::new(rank as u8, suit as u8))
    }

    /// Get the card's ID (0-51).
    #[inline]
    pub fn id(&self) -> u8 {
        self.id
    }

    /// Get the card's rank (0-12: 2-A).
    #[inline]
    pub fn rank(&self) -> u8 {
        self.id / 4
    }

    /// Get the card's suit (0-3).
    #[inline]
    pub fn suit(&self) -> u8 {
        self.id % 4
    }

    /// Get rank character for display.
    pub fn rank_char(&self) -> char {
        RANK_CHARS[self.rank() as usize]
    }

    /// Get suit character for display.
    pub fn suit_char(&self) -> char {
        SUIT_CHARS[self.suit() as usize]
    }
}

impl fmt::Display for Card {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}{}", self.rank_char(), self.suit_char())
    }
}

impl fmt::Debug for Card {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self)
    }
}

/// A player's two hole cards.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct HoleCards {
    /// First card (higher rank by convention).
    pub card1: Card,
    /// Second card.
    pub card2: Card,
}

impl HoleCards {
    /// Create hole cards, ordering by rank (higher first).
    pub fn new(card1: Card, card2: Card) -> Self {
        if card1.rank() >= card2.rank() {
            Self { card1, card2 }
        } else {
            Self {
                card1: card2,
                card2: card1,
            }
        }
    }

    /// Parse hole cards from string like "AhKs" or "Ah Ks".
    pub fn from_str(s: &str) -> Option<Self> {
        let s = s.replace(' ', "");
        if s.len() != 4 {
            return None;
        }
        let c1 = Card::from_str(&s[0..2])?;
        let c2 = Card::from_str(&s[2..4])?;
        Some(Self::new(c1, c2))
    }

    /// Check if hole cards are suited.
    pub fn is_suited(&self) -> bool {
        self.card1.suit() == self.card2.suit()
    }

    /// Check if hole cards are a pair.
    pub fn is_pair(&self) -> bool {
        self.card1.rank() == self.card2.rank()
    }

    /// Get the hand class index (0-168) for this hand.
    /// Pairs: 0-12 (22-AA)
    /// Suited: 13-90 (A2s-KQs)
    /// Offsuit: 91-168 (A2o-KQo)
    pub fn hand_class_index(&self) -> u8 {
        let r1 = self.card1.rank();
        let r2 = self.card2.rank();

        if r1 == r2 {
            // Pair: index = rank (0=22, 12=AA)
            r1
        } else if self.is_suited() {
            // Suited: higher_rank * 13 + lower_rank - (higher_rank + 1) + 13
            // Simplified: 13 + r1 * 12 - (r1 * (r1 + 1) / 2) + r2
            // Even simpler lookup: suited hands form upper triangle
            13 + (r1 as u16 * (r1 as u16 - 1) / 2 + r2 as u16) as u8
        } else {
            // Offsuit: 91 + same formula
            91 + (r1 as u16 * (r1 as u16 - 1) / 2 + r2 as u16) as u8
        }
    }

    /// Get both cards as an array.
    pub fn cards(&self) -> [Card; 2] {
        [self.card1, self.card2]
    }

    /// Check if a card conflicts with these hole cards.
    pub fn contains(&self, card: Card) -> bool {
        self.card1.id() == card.id() || self.card2.id() == card.id()
    }
}

impl fmt::Display for HoleCards {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}{}", self.card1, self.card2)
    }
}

impl fmt::Debug for HoleCards {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self)
    }
}

/// Community cards on the board.
#[derive(Clone, PartialEq, Eq, Hash, Default)]
pub struct Board {
    cards: Vec<Card>,
}

impl Board {
    /// Create an empty board.
    pub fn new() -> Self {
        Self { cards: Vec::with_capacity(5) }
    }

    /// Create a board from cards.
    pub fn from_cards(cards: Vec<Card>) -> Self {
        debug_assert!(cards.len() <= 5);
        Self { cards }
    }

    /// Parse a board from string like "AhKsQd".
    pub fn from_str(s: &str) -> Option<Self> {
        let s = s.replace(' ', "");
        if s.is_empty() {
            return Some(Self::new());
        }
        if s.len() % 2 != 0 || s.len() > 10 {
            return None;
        }

        let mut cards = Vec::with_capacity(5);
        for i in (0..s.len()).step_by(2) {
            cards.push(Card::from_str(&s[i..i + 2])?);
        }
        Some(Self::from_cards(cards))
    }

    /// Get the number of cards on the board.
    pub fn len(&self) -> usize {
        self.cards.len()
    }

    /// Check if board is empty.
    pub fn is_empty(&self) -> bool {
        self.cards.is_empty()
    }

    /// Get the cards on the board.
    pub fn cards(&self) -> &[Card] {
        &self.cards
    }

    /// Add a card to the board.
    pub fn add(&mut self, card: Card) {
        debug_assert!(self.cards.len() < 5);
        self.cards.push(card);
    }

    /// Check if the board contains a specific card.
    pub fn contains(&self, card: Card) -> bool {
        self.cards.iter().any(|&c| c.id() == card.id())
    }

    /// Get the current street based on board cards.
    pub fn street(&self) -> Street {
        match self.cards.len() {
            0 => Street::Preflop,
            3 => Street::Flop,
            4 => Street::Turn,
            5 => Street::River,
            _ => panic!("Invalid board size: {}", self.cards.len()),
        }
    }
}

impl fmt::Display for Board {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for card in &self.cards {
            write!(f, "{}", card)?;
        }
        Ok(())
    }
}

impl fmt::Debug for Board {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}]", self)
    }
}

/// Street in a poker hand.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Street {
    Preflop,
    Flop,
    Turn,
    River,
    Showdown,
}

impl Street {
    /// Get the next street.
    pub fn next(&self) -> Option<Street> {
        match self {
            Street::Preflop => Some(Street::Flop),
            Street::Flop => Some(Street::Turn),
            Street::Turn => Some(Street::River),
            Street::River => Some(Street::Showdown),
            Street::Showdown => None,
        }
    }

    /// Get street index (0-4).
    pub fn index(&self) -> usize {
        match self {
            Street::Preflop => 0,
            Street::Flop => 1,
            Street::Turn => 2,
            Street::River => 3,
            Street::Showdown => 4,
        }
    }

    /// Number of board cards for this street.
    pub fn num_board_cards(&self) -> usize {
        match self {
            Street::Preflop => 0,
            Street::Flop => 3,
            Street::Turn => 4,
            Street::River | Street::Showdown => 5,
        }
    }
}

impl fmt::Display for Street {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Street::Preflop => write!(f, "Preflop"),
            Street::Flop => write!(f, "Flop"),
            Street::Turn => write!(f, "Turn"),
            Street::River => write!(f, "River"),
            Street::Showdown => write!(f, "Showdown"),
        }
    }
}

/// A deck of 52 playing cards.
#[derive(Clone)]
pub struct Deck {
    /// All 52 cards in current order.
    cards: [Card; 52],
    /// Index of next card to deal.
    index: usize,
    /// Number of usable cards in the deck (52 minus dead cards).
    size: usize,
    /// Bitmask of dealt cards (for fast checking).
    dealt_mask: u64,
}

impl Deck {
    /// Create a new deck in standard order.
    pub fn new() -> Self {
        let mut cards = [Card::from_id(0); 52];
        for i in 0..52 {
            cards[i] = Card::from_id(i as u8);
        }
        Self {
            cards,
            index: 0,
            size: 52,
            dealt_mask: 0,
        }
    }

    /// Create a deck with specific cards removed.
    pub fn without(dead_cards: &[Card]) -> Self {
        let mut deck = Self::new();
        for &card in dead_cards {
            deck.dealt_mask |= 1u64 << card.id();
        }
        // Move non-dead cards to front
        let mut write_idx = 0;
        for read_idx in 0..52 {
            let card = Card::from_id(read_idx as u8);
            if !dead_cards.contains(&card) {
                deck.cards[write_idx] = card;
                write_idx += 1;
            }
        }
        deck.index = 0;
        deck.size = write_idx; // Track actual number of usable cards
        deck
    }

    /// Shuffle the remaining cards in the deck.
    pub fn shuffle<R: Rng>(&mut self, rng: &mut R) {
        self.cards[self.index..self.size].shuffle(rng);
    }

    /// Deal the next card from the deck.
    pub fn deal(&mut self) -> Option<Card> {
        if self.index >= self.size {
            return None;
        }
        let card = self.cards[self.index];
        self.index += 1;
        self.dealt_mask |= 1u64 << card.id();
        Some(card)
    }

    /// Deal multiple cards.
    pub fn deal_n(&mut self, n: usize) -> Vec<Card> {
        let mut cards = Vec::with_capacity(n);
        for _ in 0..n {
            if let Some(card) = self.deal() {
                cards.push(card);
            } else {
                break;
            }
        }
        cards
    }

    /// Get the number of remaining cards.
    pub fn remaining(&self) -> usize {
        self.size - self.index
    }

    /// Check if a card has been dealt.
    pub fn is_dealt(&self, card: Card) -> bool {
        self.dealt_mask & (1u64 << card.id()) != 0
    }

    /// Reset the deck to initial state.
    pub fn reset(&mut self) {
        self.index = 0;
        self.size = 52;
        self.dealt_mask = 0;
        for i in 0..52 {
            self.cards[i] = Card::from_id(i as u8);
        }
    }

    /// Get remaining cards as a slice.
    pub fn remaining_cards(&self) -> &[Card] {
        &self.cards[self.index..self.size]
    }
}

impl Default for Deck {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for Deck {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Deck({} remaining)", self.remaining())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_card_creation() {
        let ace_spades = Card::new(RANK_A, SUIT_SPADES);
        assert_eq!(ace_spades.rank(), RANK_A);
        assert_eq!(ace_spades.suit(), SUIT_SPADES);
        assert_eq!(ace_spades.to_string(), "As");

        let two_clubs = Card::new(RANK_2, SUIT_CLUBS);
        assert_eq!(two_clubs.rank(), RANK_2);
        assert_eq!(two_clubs.suit(), SUIT_CLUBS);
        assert_eq!(two_clubs.to_string(), "2c");
    }

    #[test]
    fn test_card_parsing() {
        assert_eq!(Card::from_str("As").unwrap().to_string(), "As");
        assert_eq!(Card::from_str("Kh").unwrap().to_string(), "Kh");
        assert_eq!(Card::from_str("2c").unwrap().to_string(), "2c");
        assert_eq!(Card::from_str("Td").unwrap().to_string(), "Td");
        assert!(Card::from_str("XX").is_none());
        assert!(Card::from_str("A").is_none());
    }

    #[test]
    fn test_hole_cards() {
        let hc = HoleCards::from_str("AhKs").unwrap();
        assert_eq!(hc.card1.rank(), RANK_A);
        assert_eq!(hc.card2.rank(), RANK_K);
        assert!(!hc.is_suited());
        assert!(!hc.is_pair());

        let hc_suited = HoleCards::from_str("AsKs").unwrap();
        assert!(hc_suited.is_suited());

        let hc_pair = HoleCards::from_str("AhAs").unwrap();
        assert!(hc_pair.is_pair());
    }

    #[test]
    fn test_hand_class_index() {
        // Pairs
        let aa = HoleCards::from_str("AhAs").unwrap();
        assert_eq!(aa.hand_class_index(), RANK_A); // 12

        let kk = HoleCards::from_str("KhKs").unwrap();
        assert_eq!(kk.hand_class_index(), RANK_K); // 11

        let tt = HoleCards::from_str("ThTs").unwrap();
        assert_eq!(tt.hand_class_index(), RANK_T); // 8

        // Suited hands should be in range 13-90
        let aks = HoleCards::from_str("AsKs").unwrap();
        let idx = aks.hand_class_index();
        assert!(idx >= 13 && idx <= 90, "AKs index {} should be 13-90", idx);

        // Offsuit hands should be in range 91-168
        let ako = HoleCards::from_str("AhKs").unwrap();
        let idx = ako.hand_class_index();
        assert!(idx >= 91 && idx <= 168, "AKo index {} should be 91-168", idx);
    }

    #[test]
    fn test_board() {
        let mut board = Board::new();
        assert_eq!(board.len(), 0);
        assert_eq!(board.street(), Street::Preflop);

        board = Board::from_str("AhKsQd").unwrap();
        assert_eq!(board.len(), 3);
        assert_eq!(board.street(), Street::Flop);

        board.add(Card::from_str("Jc").unwrap());
        assert_eq!(board.len(), 4);
        assert_eq!(board.street(), Street::Turn);

        board.add(Card::from_str("Tc").unwrap());
        assert_eq!(board.len(), 5);
        assert_eq!(board.street(), Street::River);
    }

    #[test]
    fn test_deck() {
        let mut deck = Deck::new();
        assert_eq!(deck.remaining(), 52);

        let card = deck.deal().unwrap();
        assert_eq!(deck.remaining(), 51);
        assert!(deck.is_dealt(card));

        // Deal remaining cards
        let cards = deck.deal_n(51);
        assert_eq!(cards.len(), 51);
        assert_eq!(deck.remaining(), 0);
        assert!(deck.deal().is_none());
    }

    #[test]
    fn test_deck_without() {
        let dead = vec![
            Card::from_str("As").unwrap(),
            Card::from_str("Ah").unwrap(),
        ];
        let deck = Deck::without(&dead);
        assert_eq!(deck.remaining(), 50);
    }

    #[test]
    fn test_street_progression() {
        assert_eq!(Street::Preflop.next(), Some(Street::Flop));
        assert_eq!(Street::Flop.next(), Some(Street::Turn));
        assert_eq!(Street::Turn.next(), Some(Street::River));
        assert_eq!(Street::River.next(), Some(Street::Showdown));
        assert_eq!(Street::Showdown.next(), None);
    }
}
