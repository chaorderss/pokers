#![allow(unused)]
#[cfg(test)]
use proptest_derive::Arbitrary;
use pyo3::prelude::*;
pub mod action;
pub mod card;
pub mod stage;
use action::{ActionEnum, ActionRecord};
use card::Card;
use stage::Stage;

#[pyclass]
#[derive(Debug, Clone)]
#[cfg_attr(test, derive(Arbitrary))]
pub struct State {
    #[pyo3(get, set)]
    pub current_player: u64,

    #[pyo3(get, set)]
    pub players_state: Vec<PlayerState>,

    #[pyo3(get, set)]
    pub public_cards: Vec<Card>,

    #[pyo3(get, set)]
    pub stage: Stage,

    #[pyo3(get, set)]
    pub button: u64,

    #[pyo3(get, set)]
    pub from_action: Option<ActionRecord>,

    #[pyo3(get, set)]
    pub legal_actions: Vec<ActionEnum>,

    #[pyo3(get, set)]
    pub deck: Vec<Card>,

    #[pyo3(get, set)]
    pub pot: f64,

    #[pyo3(get, set)]
    pub min_bet: f64,

    #[pyo3(get, set)]
    pub final_state: bool,

    #[pyo3(get, set)]
    pub status: StateStatus,
}

#[pyclass]
#[derive(Debug, Clone, Copy)]
#[cfg_attr(test, derive(Arbitrary))]
pub struct PlayerState {
    #[pyo3(get, set)]
    pub player: u64,

    #[pyo3(get, set)]
    pub hand: (Card, Card),

    #[pyo3(get, set)]
    pub bet_chips: f64,

    #[pyo3(get, set)]
    pub pot_chips: f64,

    #[pyo3(get, set)]
    pub stake: f64,

    #[pyo3(get, set)]
    pub reward: f64,

    #[pyo3(get, set)]
    pub active: bool,

    pub last_stage_action: Option<ActionEnum>,
}

#[pymethods]
impl PlayerState {
    pub fn __str__(&self) -> PyResult<String> {
        Ok(format!("{:#?}", self))
    }
}

#[pyclass]
#[derive(Debug, Clone, Copy)]
#[cfg_attr(test, derive(Arbitrary))]
pub enum StateStatus {
    Ok,
    IllegalAction,
    HighBet,
}
