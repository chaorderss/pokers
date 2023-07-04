#![allow(unused)]
#[cfg(test)]
use proptest_derive::Arbitrary;
use pyo3::prelude::*;
use strum::IntoEnumIterator;
use strum_macros::EnumIter;

#[pyclass]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(test, derive(Arbitrary))]
pub struct Card {
    #[pyo3(get, set)]
    pub suit: CardSuit,

    #[pyo3(get, set)]
    pub rank: CardRank,
}

#[pymethods]
impl Card {
    #[new]
    pub fn new(suit: CardSuit, rank: CardRank) -> Card {
        Card {
            suit: suit,
            rank: rank,
        }
    }

    #[staticmethod]
    pub fn from_string(string: String) -> Option<Card> {
        if string.len() != 2 {
            return None;
        }

        let suit = match string.to_uppercase().chars().nth(0).unwrap() {
            'C' => Some(CardSuit::Clubs),
            'D' => Some(CardSuit::Diamonds),
            'H' => Some(CardSuit::Hearts),
            'S' => Some(CardSuit::Spades),
            _ => None,
        }?;

        let rank = match string.chars().nth(1).unwrap() {
            '2' => Some(CardRank::R2),
            '3' => Some(CardRank::R3),
            '4' => Some(CardRank::R4),
            '5' => Some(CardRank::R5),
            '6' => Some(CardRank::R6),
            '7' => Some(CardRank::R7),
            '8' => Some(CardRank::R8),
            '9' => Some(CardRank::R9),
            'T' => Some(CardRank::RT),
            'J' => Some(CardRank::RJ),
            'Q' => Some(CardRank::RQ),
            'K' => Some(CardRank::RK),
            'A' => Some(CardRank::RA),
            _ => None,
        }?;

        Some(Card {
            suit: suit,
            rank: rank,
        })
    }

    #[staticmethod]
    pub fn collect() -> Vec<Card> {
        let suits: Vec<CardSuit> = CardSuit::iter().collect();
        let ranks: Vec<CardRank> = CardRank::iter().collect();
        suits
            .iter()
            .flat_map(|&s| ranks.iter().map(move |&r| Card { suit: s, rank: r }))
            .collect::<Vec<Card>>()
    }
}

impl core::fmt::Display for Card {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        let suit_symbol = match self.suit {
            CardSuit::Clubs => '♣',
            CardSuit::Diamonds => '♦',
            CardSuit::Hearts => '♥',
            CardSuit::Spades => '♠',
        };
        write!(
            f,
            "{}{}",
            format!("{:?}", self.rank).chars().nth(1).unwrap(),
            suit_symbol
        )
    }
}

#[pyclass]
#[derive(Debug, Clone, Copy, EnumIter, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(test, derive(Arbitrary))]
pub enum CardSuit {
    Clubs,
    Diamonds,
    Hearts,
    Spades,
}

#[pyclass]
#[derive(Debug, Clone, Copy, EnumIter, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(test, derive(Arbitrary))]
pub enum CardRank {
    R2,
    R3,
    R4,
    R5,
    R6,
    R7,
    R8,
    R9,
    RT,
    RJ,
    RQ,
    RK,
    RA,
}
