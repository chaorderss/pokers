use pyo3::exceptions::PyOSError;
use pyo3::prelude::*;
use rand::{seq::SliceRandom, SeedableRng};
use strum::IntoEnumIterator;
use std::cmp;

use crate::state::{Action, ActionEnum, ActionRecord, Card, PlayerState, Stage, State};

#[derive(Debug)]
pub enum ActionError {
    IllegalAction,
    LowBet,
    HighBet,
}

impl std::convert::From<ActionError> for PyErr {
    fn from(err: ActionError) -> PyErr {
        PyOSError::new_err(format!("{:?}", err))
    }
}

#[pymethods]
impl State {
    #[new]
    pub fn new(n_players: u64, button: u64, seed: u64) -> State {
        assert!(n_players >= 2);
        assert!(button < n_players);

        let mut rng = rand::rngs::StdRng::seed_from_u64(seed);
        let mut deck: Vec<Card> = Card::iter().collect();
        deck.shuffle(&mut rng);

        let mut players_state: Vec<PlayerState> = Vec::new();
        for i in 0..n_players {
            let chips = match i {
                _ if i == (button + 1) % n_players => 1,
                _ if i == (button + 2) % n_players => 2,
                _ => 0,
            };
            let p_state = PlayerState {
                player: i,
                hand: (deck.pop().unwrap(), deck.pop().unwrap()),
                bet_chips: chips,
                pot_chips: 0,
                stake: 100 - chips,
                reward: 0.0,
                active: true,
                last_stage_action: None,
            };
            players_state.push(p_state);
        }

        let mut state = State {
            current_player: (button + 3) % n_players,
            players_state: players_state,
            public_cards: Vec::new(),
            stage: Stage::Preflop,
            button: button,
            from_action: None,
            legal_actions: Vec::new(),
            deck: deck,
            final_state: false,
            pot: 3, // El pot se actualiza de forma continua (cada bet o raise) o solo cuando termina una ronda?
            min_bet: 2, 
        };

        state.legal_actions = legal_actions(&state);
        state
    }

    pub fn apply_action(&self, action: Action) -> Result<State, ActionError> {
        if self.final_state {
            return Err(ActionError::IllegalAction);
        }

        let mut new_state = self.clone();
        let player = self.current_player as usize;

        // Checkear que la acción es legal contra self.legal actions y al final meter las acciones legales para el nuevo turno
        if !self.legal_actions.contains(&action.action) {
            return Err(ActionError::IllegalAction);
        }

        match action.action {
            ActionEnum::Fold => {
                // Hay que hacer bet antes? Tiene siquiera sentido tener la acción bet, no es en realidad todo raise?
                new_state.players_state[player].active = false;
                new_state.players_state[player].pot_chips += self.players_state[player].bet_chips;
                new_state.players_state[player].bet_chips = 0;
                new_state.players_state[player].reward =
                    -(new_state.players_state[player].pot_chips as f64);

                // If all remaining players except the one betting have folded the hand ends
                let active_players: Vec<&PlayerState> = new_state
                    .players_state
                    .iter()
                    .filter(|ps| ps.active)
                    .collect();
                if active_players.len() == 1 {
                    new_state.set_winner(active_players[0].player);
                }
            }

            ActionEnum::Call => {
                let raised_chips = self.min_bet - self.players_state[player].bet_chips;
                new_state.players_state[player].bet_chips += raised_chips;
                new_state.players_state[player].stake -= raised_chips;
                new_state.pot += raised_chips;
            }

            ActionEnum::Raise => {
                if action.amount < self.min_bet - self.players_state[player].bet_chips {
                    return Err(ActionError::LowBet);
                } else if action.amount > self.players_state[player].stake {
                    return Err(ActionError::HighBet);
                }
                new_state.players_state[player].bet_chips += action.amount;
                new_state.players_state[player].stake -= action.amount;
                new_state.pot += action.amount;
                new_state.min_bet = new_state.players_state[player].bet_chips 
            }

            ActionEnum::Check => (),
        };

        new_state.from_action = Some(ActionRecord {
            player: self.current_player,
            action: action,
            stage: self.stage,
            legal_actions: self.legal_actions.clone(),
        });

        new_state.players_state[player].last_stage_action = Some(action.action);

        new_state.current_player = (self.current_player + 1) % self.players_state.len() as u64;
        while !self.players_state[new_state.current_player as usize].active {
            new_state.current_player = (new_state.current_player + 1) % self.players_state.len() as u64;
        }

        // The round ends if:
        // Every player has done call or fold
        let max_bet = new_state.players_state.iter().map(|ps| ps.bet_chips).max().unwrap();
        let mut new_stage = new_state.players_state.iter().filter(|ps| ps.active).all(|ps| {
             ps.bet_chips == max_bet || !ps.active
        });

        new_stage &= max_bet != 0;
        new_stage &= new_state.players_state.iter().all(|ps| ps.last_stage_action != None);
        // Or every player has done check
        new_stage |= new_state
            .players_state
            .iter()
            .filter(|ps| ps.active)
            .all(|ps| ps.last_stage_action == Some(ActionEnum::Check));

        if new_stage {
            new_state.to_next_stage();
        }

        new_state.legal_actions = legal_actions(&new_state);
        Ok(new_state)
    }

    fn set_winner(&mut self, winner: u64) {
        assert!(winner < self.players_state.len() as u64);

        self.players_state = self
            .players_state
            .iter()
            .map(|ps| PlayerState {
                pot_chips: ps.pot_chips + ps.bet_chips,
                bet_chips: 0,
                reward: if ps.player == winner {
                    (self.pot - ps.pot_chips) as f64
                } else {
                    -(ps.pot_chips as f64)
                },
                active: false,
                ..*ps
            })
            .collect();

        self.final_state = true;
    }

    fn to_next_stage(&mut self) {
        self.stage = match self.stage {
            Stage::Preflop => Stage::Flop,
            Stage::Flop => Stage::Turn,
            Stage::Turn => Stage::River,
            _ => Stage::Showdown,
        };
        let n_deal_cards = match self.stage {
            Stage::Flop => 3,
            Stage::Turn | Stage::River => 1,
            _ => 0,
        };
        for _ in 0..n_deal_cards {
            self.public_cards.push(self.deck.pop().unwrap())
        }
        self.players_state = self
            .players_state
            .iter()
            .map(|ps| PlayerState {
                pot_chips: ps.pot_chips + ps.bet_chips,
                bet_chips: 0,
                last_stage_action: None,
                ..*ps
            })
            .collect();

        self.min_bet = 0;

        self.current_player = (self.button + 1) % self.players_state.len() as u64;
        while !self.players_state[self.current_player as usize].active {
            self.current_player = (self.current_player + 1) % self.players_state.len() as u64;
        }


    }

    pub fn __str__(&self) -> PyResult<String> {
        Ok(format!("{:#?}", self))
    }
}

#[pyfunction]
fn legal_actions(state: &State) -> Vec<ActionEnum> {
    let mut illegal_actions: Vec<ActionEnum> = Vec::new();
    match state.stage {
        Stage::Showdown => illegal_actions.append(&mut ActionEnum::iter().collect()),
        Stage::Preflop => {
            // Only legal if its done by the big blind
            if state.current_player != (state.button + 2) % state.players_state.len() as u64 {
                illegal_actions.push(ActionEnum::Check);
            }
        }
        _ => {
            
        }
    }

    if state.min_bet == 0 {
        illegal_actions.push(ActionEnum::Call);
    }

    if state.min_bet != 0 {
        illegal_actions.push(ActionEnum::Check);
    }

    let legal_actions: Vec<ActionEnum> = ActionEnum::iter()
        .filter(|a| !illegal_actions.contains(a))
        .collect();
    legal_actions
}
