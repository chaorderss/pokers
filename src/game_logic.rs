use itertools::Itertools;
use pyo3::exceptions::PyOSError;
use pyo3::prelude::*;
use rand::{seq::SliceRandom, SeedableRng};
use strum::IntoEnumIterator;

use crate::state::action::{Action, ActionEnum, ActionRecord};
use crate::state::card::{Card, CardRank, CardSuit};
use crate::state::stage::Stage;
use crate::state::{PlayerState, State};

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
    #[staticmethod]
    pub fn from_seed(
        n_players: u64,
        button: u64,
        sb: f64,
        bb: f64,
        stake: f64,
        seed: u64,
    ) -> State {
        let mut rng = rand::rngs::StdRng::seed_from_u64(seed);
        let mut deck: Vec<Card> = Card::collect();
        deck.shuffle(&mut rng);

        State::from_deck(n_players, button, sb, bb, stake, deck)
    }

    #[staticmethod]
    pub fn from_deck(
        n_players: u64,
        button: u64,
        sb: f64,
        bb: f64,
        stake: f64,
        mut deck: Vec<Card>,
    ) -> State {
        assert!(n_players >= 2);
        assert!(button < n_players);
        assert!(deck.len() as u64 >= 2 * n_players);

        let mut players_state: Vec<PlayerState> = Vec::new();
        for i in 0..n_players {
            let chips = match i {
                _ if i == (button + 1) % n_players => sb,
                _ if i == (button + 2) % n_players => bb,
                _ => 0.0,
            };

            let p_state = PlayerState {
                player: (button + i + 1) % n_players,
                hand: (deck.remove(0), deck.remove(0)),
                bet_chips: chips,
                pot_chips: 0.0,
                stake: stake - chips,
                reward: 0.0,
                active: true,
                last_stage_action: None,
            };
            players_state.push(p_state);
        }

        players_state.sort_by_key(|ps| ps.player);

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
            pot: sb + bb,
            min_bet: bb,
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

        if !self.legal_actions.contains(&action.action) {
            return Err(ActionError::IllegalAction);
        }

        match action.action {
            ActionEnum::Fold => {
                new_state.players_state[player].active = false;
                new_state.players_state[player].pot_chips += self.players_state[player].bet_chips;
                new_state.players_state[player].bet_chips = 0.0;
                new_state.players_state[player].reward =
                    -(new_state.players_state[player].pot_chips as f64);
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
            new_state.current_player =
                (new_state.current_player + 1) % self.players_state.len() as u64;
        }

        // The betting round ends if:
        // Theres two or more active players
        let active_players: Vec<PlayerState> = new_state
            .players_state
            .iter()
            .copied()
            .filter(|ps| ps.active)
            .collect();
        let multiple_active = active_players.len() >= 2;
        // Every active player has done an action
        let is_last_player = active_players.iter().all(|ps| ps.last_stage_action != None);
        // And every active player has bet the same amount
        let all_same_bet = active_players
            .iter()
            .all(|ps| ps.bet_chips == new_state.min_bet);

        let round_ended = multiple_active && is_last_player && all_same_bet;

        if round_ended {
            new_state.to_next_stage();
        }

        // The game ends if the players have reached the showdown or every player except one has folded
        if active_players.len() == 1 {
            new_state.set_winners(vec![active_players[0].player]);
        }

        if new_state.stage == Stage::Showdown {
            let ranks: Vec<(u64, u64, u64)> = active_players
                .iter()
                .map(|ps| rank_hand(ps.hand, &new_state.public_cards))
                .collect();
            let min_rank = ranks.iter().copied().min().unwrap();
            println!("Ranks: {:?}", ranks);
            let winners_indices: Vec<usize> = ranks
                .iter()
                .enumerate()
                .filter(|(_, &r)| r == min_rank)
                .map(|(i, _)| i)
                .collect();
            println!("Winner id: {:?}", winners_indices);
            new_state.set_winners(
                winners_indices
                    .iter()
                    .map(|&i| active_players[i].player)
                    .collect(),
            );
        }

        new_state.legal_actions = legal_actions(&new_state);
        Ok(new_state)
    }

    fn set_winners(&mut self, winners: Vec<u64>) {
        assert!(winners.iter().all(|&p| p < self.players_state.len() as u64));

        let winner_reward = self
            .players_state
            .iter()
            .filter(|&&ps| !winners.contains(&ps.player))
            .map(|ps| ps.pot_chips + ps.bet_chips)
            .fold(0.0, |c1, c2| c1 + c2)
            / winners.len() as f64;

        self.players_state = self
            .players_state
            .iter()
            .map(|ps| PlayerState {
                pot_chips: 0.0,
                bet_chips: 0.0,
                reward: if winners.contains(&ps.player) {
                    winner_reward
                } else {
                    -(ps.pot_chips + ps.bet_chips as f64)
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
            self.public_cards.push(self.deck.remove(0))
        }
        self.players_state = self
            .players_state
            .iter()
            .map(|ps| PlayerState {
                pot_chips: ps.pot_chips + ps.bet_chips,
                bet_chips: 0.0,
                last_stage_action: None,
                ..*ps
            })
            .collect();

        self.min_bet = 0.0;

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
        _ => {}
    }

    if state.min_bet == 0.0 {
        illegal_actions.push(ActionEnum::Call);
    }

    if state.min_bet != 0.0 {
        illegal_actions.push(ActionEnum::Check);
    }

    let legal_actions: Vec<ActionEnum> = ActionEnum::iter()
        .filter(|a| !illegal_actions.contains(a))
        .collect();
    legal_actions
}

fn rank_hand(private_cards: (Card, Card), public_cards: &Vec<Card>) -> (u64, u64, u64) {
    let mut cards = public_cards.clone();
    cards.append(&mut vec![private_cards.0, private_cards.1]);

    let min_rank = cards
        .iter()
        .copied()
        .combinations(5)
        .map(|comb| rank_card_combination(comb))
        .min()
        .unwrap();

    min_rank
}

fn rank_card_combination(cards: Vec<Card>) -> (u64, u64, u64) {
    let mut ordered_cards = cards.clone();
    ordered_cards.sort_by_key(|c| c.rank);
    let suits: Vec<CardSuit> = ordered_cards.iter().map(|c| c.suit).collect();
    let ranks: Vec<CardRank> = ordered_cards.iter().map(|c| c.rank).collect();

    let suit_duplicates: Vec<(usize, CardSuit)> = suits
        .iter()
        .copied()
        .dedup_with_count()
        .sorted_by_key(|(n, _)| n.clone())
        .rev()
        .collect();

    let rank_duplicates: Vec<(usize, CardRank)> = ranks
        .iter()
        .copied()
        .dedup_with_count()
        .sorted_by_key(|(n, _)| n.clone())
        .rev()
        .collect();

    let ranks_in_sequence = ranks
        .windows(2)
        .map(|x| x[1] as i32 - x[0] as i32)
        .all(|d| d == 1)
        || ranks
            == vec![
                CardRank::R2,
                CardRank::R3,
                CardRank::R4,
                CardRank::R5,
                CardRank::RA,
            ];

    // Royal flush: A, K, Q, J, 10, all the same suit.
    if ranks[..]
        == [
            CardRank::RT,
            CardRank::RJ,
            CardRank::RQ,
            CardRank::RK,
            CardRank::RA,
        ]
        && suit_duplicates[0].0 == 5
    {
        return (1, 0, 0_u64);
    }
    // Straight flush: Five cards in a sequence, all in the same suit.
    if ranks_in_sequence && suit_duplicates[0].0 == 5 {
        return (2, high_card_value(&ranks), 0_u64);
    }
    // 3. Four of a kind: All four cards of the same rank.
    if rank_duplicates[0].0 == 4 {
        let relevant_ranks = vec![rank_duplicates[0].1];
        return (3, high_card_value(&relevant_ranks), high_card_value(&ranks));
    }
    // 4. Full house: Three of a kind with a pair.
    if rank_duplicates[0].0 == 3 && rank_duplicates[1].0 == 2 {
        let relevant_ranks = vec![rank_duplicates[0].1];
        return (4, high_card_value(&relevant_ranks), high_card_value(&ranks));
    }
    // 5. Flush: Any five cards of the same suit, but not in a sequence.
    if suit_duplicates[0].0 == 5 {
        return (5, high_card_value(&ranks), 0_u64);
    }
    // 6. Straight: Five cards in a sequence, but not of the same suit.
    if ranks_in_sequence {
        return (6, high_card_value(&ranks), 0_u64);
    }
    // 7. Three of a kind: Three cards of the same rank.
    if rank_duplicates[0].0 == 3 {
        let relevant_ranks = vec![rank_duplicates[0].1];
        return (7, high_card_value(&relevant_ranks), high_card_value(&ranks));
    }
    // 8. Two pair: Two different pairs.
    if rank_duplicates[0].0 == 2 && rank_duplicates[1].0 == 2 {
        let relevant_ranks = vec![rank_duplicates[0].1, rank_duplicates[1].1];
        return (8, high_card_value(&relevant_ranks), high_card_value(&ranks));
    }
    // 9. Pair: Two cards of the same rank.
    if rank_duplicates[0].0 == 2 {
        let relevant_ranks = vec![rank_duplicates[0].1];
        return (9, high_card_value(&relevant_ranks), high_card_value(&ranks));
    }

    // 10. High Card: When you haven't made any of the hands above, the highest card plays.
    (10, high_card_value(&ranks), 0_u64)
}

fn high_card_value(ranks: &Vec<CardRank>) -> u64 {
    let mut value: u64 = 0;
    for (i, &r) in ranks.iter().sorted().enumerate() {
        value += (13_u64.pow(i as u32)) * (12 - r as u64);
    }
    value
}
