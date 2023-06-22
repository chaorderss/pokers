use pyo3::prelude::*;
use strum_macros::EnumIter;

#[pyclass]
#[derive(Debug, Clone)]
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
    pub pot: u64,

    #[pyo3(get, set)]
    pub min_bet: u64,

    #[pyo3(get, set)]
    pub final_state: bool,
}

#[pyclass]
#[derive(Debug, Clone, Copy)]
pub struct PlayerState {
    #[pyo3(get, set)]
    pub player: u64,

    #[pyo3(get, set)]
    pub hand: (Card, Card),

    #[pyo3(get, set)]
    pub bet_chips: u64,

    #[pyo3(get, set)]
    pub pot_chips: u64,

    #[pyo3(get, set)]
    pub stake: u64,

    #[pyo3(get, set)]
    pub reward: f64,

    #[pyo3(get, set)]
    pub active: bool,

    pub last_stage_action: Option<ActionEnum>,
}

#[pyclass]
#[derive(Debug, Clone)]
pub struct ActionRecord {
    #[pyo3(get, set)]
    pub player: u64,

    #[pyo3(get, set)]
    pub stage: Stage,

    #[pyo3(get, set)]
    pub action: Action,

    #[pyo3(get, set)]
    pub legal_actions: Vec<ActionEnum>,
}

#[pyclass]
#[derive(Debug, Clone, Copy, EnumIter, PartialEq, Eq)]
pub enum ActionEnum {
    Fold,
    Check,
    Call,
    Raise,
}

#[pyclass]
#[derive(Debug, Clone, Copy)]
pub struct Action {
    #[pyo3(get, set)]
    pub action: ActionEnum,

    #[pyo3(get, set)]
    pub amount: u64,
}

#[pymethods]
impl Action {
    #[new]
    #[pyo3(signature = (action, amount=0))]
    pub fn new(action: ActionEnum, amount: u64) -> Action {
        Action {
            action: action,
            amount: amount,
        }
    }
}

#[pyclass]
#[derive(Debug, Clone, Copy, EnumIter)]
pub enum Stage {
    Preflop,
    Flop,
    Turn,
    River,
    Showdown,
}

#[pyclass]
#[derive(Debug, Clone, Copy, EnumIter)]
pub enum Card {
    CA,
    C2,
    C3,
    C4,
    C5,
    C6,
    C7,
    C8,
    C9,
    CT,
    CJ,
    CQ,
    CK,
    DA,
    D2,
    D3,
    D4,
    D5,
    D6,
    D7,
    D8,
    D9,
    DT,
    DJ,
    DQ,
    DK,
    HA,
    H2,
    H3,
    H4,
    H5,
    H6,
    H7,
    H8,
    H9,
    HT,
    HJ,
    HQ,
    HK,
    SA,
    S2,
    S3,
    S4,
    S5,
    S6,
    S7,
    S8,
    S9,
    ST,
    SJ,
    SQ,
    SK,
}
