//! Poker hand evaluation.
//!
//! This module provides hand ranking and comparison for 5-7 card poker hands.
//! The evaluator uses a combination of bit manipulation and direct calculation
//! for fast hand ranking.

use super::card::{Card, HoleCards, Board};
use std::cmp::Ordering;

/// Hand rank categories, ordered from worst to best.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum HandCategory {
    HighCard = 0,
    OnePair = 1,
    TwoPair = 2,
    ThreeOfAKind = 3,
    Straight = 4,
    Flush = 5,
    FullHouse = 6,
    FourOfAKind = 7,
    StraightFlush = 8,
}

impl HandCategory {
    /// Get the category name.
    pub fn name(&self) -> &'static str {
        match self {
            HandCategory::HighCard => "High Card",
            HandCategory::OnePair => "One Pair",
            HandCategory::TwoPair => "Two Pair",
            HandCategory::ThreeOfAKind => "Three of a Kind",
            HandCategory::Straight => "Straight",
            HandCategory::Flush => "Flush",
            HandCategory::FullHouse => "Full House",
            HandCategory::FourOfAKind => "Four of a Kind",
            HandCategory::StraightFlush => "Straight Flush",
        }
    }
}

/// A hand rank that can be compared.
/// Higher values are better hands.
/// Format: category (4 bits) | kicker1 (4 bits) | kicker2 (4 bits) | ...
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct HandRank(u32);

impl HandRank {
    /// Create a new hand rank.
    fn new(category: HandCategory, kickers: &[u8]) -> Self {
        let mut value = (category as u32) << 20;
        for (i, &k) in kickers.iter().take(5).enumerate() {
            value |= (k as u32) << (16 - i * 4);
        }
        Self(value)
    }

    /// Get the raw rank value for comparison.
    pub fn value(&self) -> u32 {
        self.0
    }

    /// Get the hand category.
    pub fn category(&self) -> HandCategory {
        match self.0 >> 20 {
            0 => HandCategory::HighCard,
            1 => HandCategory::OnePair,
            2 => HandCategory::TwoPair,
            3 => HandCategory::ThreeOfAKind,
            4 => HandCategory::Straight,
            5 => HandCategory::Flush,
            6 => HandCategory::FullHouse,
            7 => HandCategory::FourOfAKind,
            8 => HandCategory::StraightFlush,
            _ => HandCategory::HighCard,
        }
    }
}

impl PartialOrd for HandRank {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for HandRank {
    fn cmp(&self, other: &Self) -> Ordering {
        self.0.cmp(&other.0)
    }
}

/// Hand evaluator for poker hands.
#[derive(Debug, Clone, Default)]
pub struct HandEvaluator;

impl HandEvaluator {
    /// Create a new hand evaluator.
    pub fn new() -> Self {
        Self
    }

    /// Evaluate a 5-card hand.
    pub fn evaluate_5(&self, cards: &[Card; 5]) -> HandRank {
        // Build rank counts and suit counts
        let mut rank_counts = [0u8; 13];
        let mut suit_counts = [0u8; 4];
        let mut rank_bits = 0u16; // Bitmask of ranks present

        for card in cards {
            rank_counts[card.rank() as usize] += 1;
            suit_counts[card.suit() as usize] += 1;
            rank_bits |= 1 << card.rank();
        }

        // Check for flush
        let is_flush = suit_counts.iter().any(|&c| c >= 5);

        // Check for straight
        let straight_high = self.find_straight(rank_bits);
        let is_straight = straight_high.is_some();

        // Straight flush
        if is_flush && is_straight {
            return HandRank::new(HandCategory::StraightFlush, &[straight_high.unwrap()]);
        }

        // Categorize by rank counts
        let mut quads = Vec::new();
        let mut trips = Vec::new();
        let mut pairs = Vec::new();
        let mut singles = Vec::new();

        for rank in (0..13u8).rev() {
            match rank_counts[rank as usize] {
                4 => quads.push(rank),
                3 => trips.push(rank),
                2 => pairs.push(rank),
                1 => singles.push(rank),
                _ => {}
            }
        }

        // Four of a kind
        if !quads.is_empty() {
            let kicker = trips.first()
                .or(pairs.first())
                .or(singles.first())
                .copied()
                .unwrap_or(0);
            return HandRank::new(HandCategory::FourOfAKind, &[quads[0], kicker]);
        }

        // Full house
        if !trips.is_empty() && (!pairs.is_empty() || trips.len() > 1) {
            let pair_rank = if trips.len() > 1 {
                trips[1]
            } else {
                pairs[0]
            };
            return HandRank::new(HandCategory::FullHouse, &[trips[0], pair_rank]);
        }

        // Flush
        if is_flush {
            let flush_suit = suit_counts.iter().position(|&c| c >= 5).unwrap() as u8;
            let mut flush_ranks: Vec<u8> = cards.iter()
                .filter(|c| c.suit() == flush_suit)
                .map(|c| c.rank())
                .collect();
            flush_ranks.sort_by(|a, b| b.cmp(a));
            return HandRank::new(HandCategory::Flush, &flush_ranks);
        }

        // Straight
        if is_straight {
            return HandRank::new(HandCategory::Straight, &[straight_high.unwrap()]);
        }

        // Three of a kind
        if !trips.is_empty() {
            let kickers: Vec<u8> = pairs.iter().chain(singles.iter()).take(2).copied().collect();
            return HandRank::new(HandCategory::ThreeOfAKind, &[trips[0], kickers.get(0).copied().unwrap_or(0), kickers.get(1).copied().unwrap_or(0)]);
        }

        // Two pair
        if pairs.len() >= 2 {
            let kicker = pairs.get(2).or(singles.first()).copied().unwrap_or(0);
            return HandRank::new(HandCategory::TwoPair, &[pairs[0], pairs[1], kicker]);
        }

        // One pair
        if pairs.len() == 1 {
            let kickers: Vec<u8> = singles.iter().take(3).copied().collect();
            return HandRank::new(HandCategory::OnePair, &[pairs[0],
                kickers.get(0).copied().unwrap_or(0),
                kickers.get(1).copied().unwrap_or(0),
                kickers.get(2).copied().unwrap_or(0)]);
        }

        // High card
        HandRank::new(HandCategory::HighCard, &singles)
    }

    /// Evaluate a 7-card hand (best 5-card combination).
    pub fn evaluate_7(&self, cards: &[Card; 7]) -> HandRank {
        let mut best = HandRank(0);

        // Try all 21 combinations of 5 cards from 7
        for i in 0..7 {
            for j in (i+1)..7 {
                for k in (j+1)..7 {
                    for l in (k+1)..7 {
                        for m in (l+1)..7 {
                            let hand = [cards[i], cards[j], cards[k], cards[l], cards[m]];
                            let rank = self.evaluate_5(&hand);
                            if rank > best {
                                best = rank;
                            }
                        }
                    }
                }
            }
        }

        best
    }

    /// Evaluate hole cards against a board.
    /// For incomplete boards (less than 5 cards total), returns a placeholder rank.
    pub fn evaluate(&self, hole_cards: &HoleCards, board: &Board) -> HandRank {
        let board_cards = board.cards();
        let total = 2 + board_cards.len();

        if total < 5 {
            // For incomplete boards, we can't fully evaluate
            // Return a rank based on just the hole cards (useful for preflop)
            // Higher pair = better rank
            if hole_cards.is_pair() {
                HandRank::new(HandCategory::OnePair, &[hole_cards.card1.rank(), 0, 0, 0, 0])
            } else {
                HandRank::new(HandCategory::HighCard, &[hole_cards.card1.rank(), hole_cards.card2.rank(), 0, 0, 0])
            }
        } else if total == 5 {
            let cards = [
                hole_cards.card1,
                hole_cards.card2,
                board_cards[0],
                board_cards[1],
                board_cards[2],
            ];
            self.evaluate_5(&cards)
        } else if total == 6 {
            let cards = [
                hole_cards.card1,
                hole_cards.card2,
                board_cards[0],
                board_cards[1],
                board_cards[2],
                board_cards[3],
            ];
            self.evaluate_6(&cards)
        } else if total == 7 {
            let cards = [
                hole_cards.card1,
                hole_cards.card2,
                board_cards[0],
                board_cards[1],
                board_cards[2],
                board_cards[3],
                board_cards[4],
            ];
            self.evaluate_7(&cards)
        } else {
            panic!("Invalid number of cards: {}", total);
        }
    }

    /// Evaluate a 6-card hand (best 5-card combination).
    fn evaluate_6(&self, cards: &[Card; 6]) -> HandRank {
        let mut best = HandRank(0);

        // Try all 6 combinations of 5 cards from 6
        for skip in 0..6 {
            let hand: Vec<Card> = cards.iter()
                .enumerate()
                .filter(|&(i, _)| i != skip)
                .map(|(_, &c)| c)
                .collect();
            let rank = self.evaluate_5(&[hand[0], hand[1], hand[2], hand[3], hand[4]]);
            if rank > best {
                best = rank;
            }
        }

        best
    }

    /// Find the highest straight from a rank bitmask.
    /// Returns the high card of the straight, or None if no straight.
    fn find_straight(&self, rank_bits: u16) -> Option<u8> {
        // Add ace-low bit for wheel straight (A-2-3-4-5)
        let bits = rank_bits | ((rank_bits >> 12) & 1);

        // Check from ace-high down
        for high in (4..=14).rev() {
            let mask = if high == 4 {
                // Wheel: A-2-3-4-5 = bits 12,0,1,2,3
                0b1_0000_0000_1111u16
            } else {
                0b11111u16 << (high - 4)
            };

            if (bits & mask) == mask {
                // For wheel, high card is 5 (index 3)
                return Some(if high == 4 { 3 } else { high as u8 - 2 });
            }
        }

        None
    }

    /// Compare two hands. Returns positive if hand1 wins, negative if hand2 wins, 0 for tie.
    pub fn compare(&self, hole1: &HoleCards, hole2: &HoleCards, board: &Board) -> i32 {
        let rank1 = self.evaluate(hole1, board);
        let rank2 = self.evaluate(hole2, board);

        match rank1.cmp(&rank2) {
            Ordering::Greater => 1,
            Ordering::Less => -1,
            Ordering::Equal => 0,
        }
    }
}

/// Calculate equity of hole cards against a range on a given board.
/// Returns equity as a fraction (0.0 to 1.0).
pub fn calculate_equity_vs_random(hole_cards: &HoleCards, board: &Board, samples: usize) -> f64 {
    use rand::SeedableRng;
    use rand::rngs::StdRng;

    let evaluator = HandEvaluator::new();
    let mut rng = StdRng::from_entropy();
    let mut wins = 0.0;
    let mut total = 0.0;

    // Build list of dead cards
    let dead: Vec<Card> = hole_cards.cards().iter()
        .chain(board.cards().iter())
        .copied()
        .collect();

    for _ in 0..samples {
        // Deal opponent's hand and remaining board
        let mut deck = super::card::Deck::without(&dead);
        deck.shuffle(&mut rng);

        // Deal opponent's two cards
        let opp1 = deck.deal().unwrap();
        let opp2 = deck.deal().unwrap();
        let opp_hand = HoleCards::new(opp1, opp2);

        // Complete the board
        let mut full_board = board.clone();
        while full_board.len() < 5 {
            full_board.add(deck.deal().unwrap());
        }

        // Compare hands
        let result = evaluator.compare(hole_cards, &opp_hand, &full_board);
        if result > 0 {
            wins += 1.0;
        } else if result == 0 {
            wins += 0.5;
        }
        total += 1.0;
    }

    wins / total
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cards_from_str(s: &str) -> Vec<Card> {
        let s = s.replace(' ', "");
        let mut cards = Vec::new();
        for i in (0..s.len()).step_by(2) {
            cards.push(Card::from_str(&s[i..i+2]).unwrap());
        }
        cards
    }

    fn arr5(cards: &[Card]) -> [Card; 5] {
        [cards[0], cards[1], cards[2], cards[3], cards[4]]
    }

    #[test]
    fn test_high_card() {
        let eval = HandEvaluator::new();
        let cards = cards_from_str("As Kd Qh Jc 9s");
        let rank = eval.evaluate_5(&arr5(&cards));
        assert_eq!(rank.category(), HandCategory::HighCard);
    }

    #[test]
    fn test_one_pair() {
        let eval = HandEvaluator::new();
        let cards = cards_from_str("As Ad Kh Qc Js");
        let rank = eval.evaluate_5(&arr5(&cards));
        assert_eq!(rank.category(), HandCategory::OnePair);
    }

    #[test]
    fn test_two_pair() {
        let eval = HandEvaluator::new();
        let cards = cards_from_str("As Ad Kh Kc Js");
        let rank = eval.evaluate_5(&arr5(&cards));
        assert_eq!(rank.category(), HandCategory::TwoPair);
    }

    #[test]
    fn test_three_of_a_kind() {
        let eval = HandEvaluator::new();
        let cards = cards_from_str("As Ad Ah Kc Js");
        let rank = eval.evaluate_5(&arr5(&cards));
        assert_eq!(rank.category(), HandCategory::ThreeOfAKind);
    }

    #[test]
    fn test_straight() {
        let eval = HandEvaluator::new();

        // Regular straight
        let cards = cards_from_str("Ts 9d 8h 7c 6s");
        let rank = eval.evaluate_5(&arr5(&cards));
        assert_eq!(rank.category(), HandCategory::Straight);

        // Broadway
        let cards = cards_from_str("As Kd Qh Jc Ts");
        let rank = eval.evaluate_5(&arr5(&cards));
        assert_eq!(rank.category(), HandCategory::Straight);

        // Wheel (A-2-3-4-5)
        let cards = cards_from_str("5s 4d 3h 2c As");
        let rank = eval.evaluate_5(&arr5(&cards));
        assert_eq!(rank.category(), HandCategory::Straight);
    }

    #[test]
    fn test_flush() {
        let eval = HandEvaluator::new();
        let cards = cards_from_str("As Ks 9s 7s 2s");
        let rank = eval.evaluate_5(&arr5(&cards));
        assert_eq!(rank.category(), HandCategory::Flush);
    }

    #[test]
    fn test_full_house() {
        let eval = HandEvaluator::new();
        let cards = cards_from_str("As Ad Ah Kc Kd");
        let rank = eval.evaluate_5(&arr5(&cards));
        assert_eq!(rank.category(), HandCategory::FullHouse);
    }

    #[test]
    fn test_four_of_a_kind() {
        let eval = HandEvaluator::new();
        let cards = cards_from_str("As Ad Ah Ac Ks");
        let rank = eval.evaluate_5(&arr5(&cards));
        assert_eq!(rank.category(), HandCategory::FourOfAKind);
    }

    #[test]
    fn test_straight_flush() {
        let eval = HandEvaluator::new();

        // Regular straight flush
        let cards = cards_from_str("9s 8s 7s 6s 5s");
        let rank = eval.evaluate_5(&arr5(&cards));
        assert_eq!(rank.category(), HandCategory::StraightFlush);

        // Royal flush
        let cards = cards_from_str("As Ks Qs Js Ts");
        let rank = eval.evaluate_5(&arr5(&cards));
        assert_eq!(rank.category(), HandCategory::StraightFlush);
    }

    #[test]
    fn test_hand_comparison() {
        let eval = HandEvaluator::new();

        // AA vs KK on a dry board - AA should win
        let aa = HoleCards::from_str("AhAd").unwrap();
        let kk = HoleCards::from_str("KhKd").unwrap();
        let board = Board::from_str("Qs Jc 7d 3s 2h").unwrap();

        assert!(eval.compare(&aa, &kk, &board) > 0);
        assert!(eval.compare(&kk, &aa, &board) < 0);
    }

    #[test]
    fn test_7_card_evaluation() {
        let eval = HandEvaluator::new();

        // Should find the best 5-card hand
        let hole = HoleCards::from_str("AhAs").unwrap();
        let board = Board::from_str("Ad Ac Kh Qs Jd").unwrap();

        let rank = eval.evaluate(&hole, &board);
        assert_eq!(rank.category(), HandCategory::FourOfAKind);
    }

    #[test]
    fn test_equity_calculation() {
        // AA should have high equity vs random
        let aa = HoleCards::from_str("AhAs").unwrap();
        let board = Board::new();
        let equity = calculate_equity_vs_random(&aa, &board, 1000);
        assert!(equity > 0.8, "AA equity {} should be > 80%", equity);

        // 72o should have low equity vs random
        let low = HoleCards::from_str("7h2s").unwrap();
        let equity = calculate_equity_vs_random(&low, &board, 1000);
        assert!(equity < 0.4, "72o equity {} should be < 40%", equity);
    }
}
