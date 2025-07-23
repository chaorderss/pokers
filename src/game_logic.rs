// game_logic.rs
use itertools::Itertools;
use pyo3::exceptions::PyOSError;
use pyo3::prelude::*;
use rand::{seq::SliceRandom, SeedableRng};
use strum::IntoEnumIterator;

use crate::state::action::{Action, ActionEnum, ActionRecord};
use crate::state::card::{Card, CardRank, CardSuit};
use crate::state::stage::Stage;
use crate::state::{PlayerState, State, StateStatus};

// Define a macro for verbose printing controlled by environment variable
macro_rules! verbose_println {
    ($state:expr, $($arg:tt)*) => {
        if $state.verbose && std::env::var("POKERS_VERBOSE").is_ok() {
            println!($($arg)*);
        }
    };
}

pub struct InitStateError {
    msg: String,
}

impl std::convert::From<InitStateError> for PyErr {
    fn from(err: InitStateError) -> PyErr {
        PyOSError::new_err(err.msg)
    }
}

#[pymethods]
impl State {
    #[staticmethod]
    #[pyo3(signature = (n_players, button, sb, bb, stake, seed, verbose=false))]
    pub fn from_seed(
        n_players: u64,
        button: u64,
        sb: f64,
        bb: f64,
        stake: f64,
        seed: u64,
        verbose: bool,
    ) -> Result<State, InitStateError> {
        let mut rng = rand::rngs::StdRng::seed_from_u64(seed);
        let mut deck: Vec<Card> = Card::collect();
        deck.shuffle(&mut rng);

        State::from_deck(n_players, button, sb, bb, stake, deck, verbose)
    }

    #[staticmethod]
    #[pyo3(signature = (n_players, button, sb, bb, stake, deck, verbose=false))]
    pub fn from_deck(
        n_players: u64,
        button: u64,
        sb: f64,
        bb: f64,
        stake: f64,
        mut deck: Vec<Card>,
        verbose: bool,
    ) -> Result<State, InitStateError> {
        if n_players < 2 {
            return Err(InitStateError {
                msg: "The number of players must be 2 or more".to_owned(),
            });
        }

        if button >= n_players {
            return Err(InitStateError {
                msg: "The button must be between the players".to_owned(),
            });
        }

        if deck.len() < 2 * n_players as usize {
            return Err(InitStateError {
                msg: "The number of cards in the deck must be at least 2*n_players".to_owned(),
            });
        }

        if sb < 0.0 {
            return Err(InitStateError {
                msg: "The small blind must be greater than 0".to_owned(),
            });
        }

        if bb < sb {
            return Err(InitStateError {
                msg: "The small blind must be smaller or equal than the big blind".to_owned(),
            });
        }

        if stake < bb {
            return Err(InitStateError {
                msg: "The stake must be greater or equal than the big blind".to_owned(),
            });
        }

        let mut players_state: Vec<PlayerState> = Vec::new();
        for i in 0..n_players {
            let player = (button + i + 1) % n_players;
            let chips = match i {
                _ if player == (button + 1) % n_players => sb,
                _ if player == (button + 2) % n_players => bb,
                _ => 0.0,
            };

            let p_state = PlayerState {
                player: player,
                hand: (deck.remove(0), deck.remove(0)),
                bet_chips: chips,
                pot_chips: 0.0,
                stake: stake - chips,
                reward: 0.0,
                active: true,
                range_idx: -1, // Will be calculated later
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
            action_list: Vec::new(),
            legal_actions: Vec::new(),
            deck: deck,
            final_state: false,
            pot: sb + bb,
            min_bet: bb,
            status: StateStatus::Ok,
            verbose: verbose,
        };

        // Update range indices for all players
        state.update_range_indices();

        state.legal_actions = legal_actions(&state);
        Ok(state)
    }

    pub fn apply_action(&self, action: Action) -> State {
        // Define MIN_MEANINGFUL_STAKE at the top of the function for consistent use
        const MIN_MEANINGFUL_STAKE: f64 = 1.0;

        match self.status {
            StateStatus::Ok => (),
            _ => return self.clone(),
        }

        if self.final_state {
            return self.clone();
        }

        // If we're already at showdown, the game is over - no actions allowed
        if self.stage == Stage::Showdown {
            return State {
                status: StateStatus::IllegalAction,
                final_state: true,
                ..self.clone()
            };
        }

        let mut new_state = self.clone();
        let action_record = ActionRecord {
            player: self.current_player,
            action: action,
            stage: self.stage,
            legal_actions: self.legal_actions.clone(),
        };
        new_state.from_action = Some(action_record.clone());
        new_state.action_list.push(action_record);

        if !self.legal_actions.contains(&action.action) {
            return State {
                status: StateStatus::IllegalAction,
                final_state: true,
                ..new_state
            };
        }

        let player = self.current_player as usize;

        match action.action {
            ActionEnum::Fold => {
                new_state.players_state[player].active = false;
                new_state.players_state[player].pot_chips += self.players_state[player].bet_chips;
                new_state.players_state[player].bet_chips = 0.0;
                new_state.players_state[player].reward =
                    -(new_state.players_state[player].pot_chips as f64);
            }

            ActionEnum::CheckCall => {
                // If min_bet is 0 (no bet to call), this acts as Check
                if self.min_bet == 0.0 || self.min_bet <= self.players_state[player].bet_chips {
                    // Check - no action needed
                } else {
                    // Call - match the minimum bet
                    let required_chips = self.min_bet - self.players_state[player].bet_chips;
                    let actual_chips = if required_chips > self.players_state[player].stake {
                        // Player doesn't have enough chips to call - go all-in instead
                        self.players_state[player].stake
                    } else {
                        required_chips
                    };

                    new_state.players_state[player].bet_chips += actual_chips;
                    new_state.players_state[player].stake -= actual_chips;
                    new_state.pot += actual_chips;
                }
            }

            ActionEnum::BetRaise => {
                // If min_bet is 0, this acts as Bet; otherwise, it's a Raise
                let bet = if self.min_bet == 0.0 {
                    // First bet (Bet)
                    action.amount
                } else {
                    // Raise over the minimum bet
                    (self.min_bet - self.players_state[player].bet_chips) + action.amount
                };

                let actual_bet = if bet > self.players_state[player].stake {
                    // Player wants to bet more than they have - go all-in instead
                    self.players_state[player].stake
                } else {
                    bet
                };

                new_state.players_state[player].bet_chips += actual_bet;
                new_state.players_state[player].stake -= actual_bet;
                new_state.pot += actual_bet;

                // Update min_bet only if this player's bet is higher than current min_bet
                if new_state.players_state[player].bet_chips > new_state.min_bet {
                    new_state.min_bet = new_state.players_state[player].bet_chips;
                }
            }
        };

        new_state.players_state[player].last_stage_action = Some(action.action);

        // Check if all active players are all-in (have no remaining stake)
        // CRITICAL: Immediately check if all active players are all-in after any action
        let active_players: Vec<PlayerState> = new_state
            .players_state
            .iter()
            .copied()
            .filter(|ps| ps.active)
            .collect();

        // CRITICAL FIX: Players have meaningful stake only if their remaining chips
        // are significant both in absolute terms AND relative to current betting level
        let players_with_stake: Vec<&PlayerState> = active_players
            .iter()
            .filter(|ps| {
                // Absolute threshold: must have at least MIN_MEANINGFUL_STAKE
                if ps.stake < MIN_MEANINGFUL_STAKE {
                    return false;
                }

                // Relative threshold: remaining stake should be meaningful relative to current bet
                // If they've already bet significantly, remaining chips should be substantial
                if ps.bet_chips > 0.0 {
                    // Player's remaining stake should be at least 5% of what they've already bet
                    // OR enough to make a meaningful raise (whichever is larger)
                    let min_relative_stake = (ps.bet_chips * 0.05).max(MIN_MEANINGFUL_STAKE);
                    ps.stake >= min_relative_stake
                } else {
                    // No bet yet, just need absolute threshold
                    true
                }
            })
            .collect();

        if !active_players.is_empty()
            && players_with_stake.is_empty()
            && new_state.stage != Stage::Showdown
        {
            // Skip to showdown stage
            while new_state.stage != Stage::Showdown {
                new_state.to_next_stage();
            }

            // Determine winners and end the game
            let ranks: Vec<(u64, u64, u64)> = active_players
                .iter()
                .map(|ps| rank_hand(&new_state, ps.hand, &new_state.public_cards))
                .collect();
            let min_rank = ranks.iter().copied().min().unwrap();

            let winners_indices: Vec<usize> = ranks
                .iter()
                .enumerate()
                .filter(|(_, &r)| r == min_rank)
                .map(|(i, _)| i)
                .collect();

            new_state.set_winners(
                winners_indices
                    .iter()
                    .map(|&i| active_players[i].player)
                    .collect(),
            );

            return new_state;
        }

        // Existing logic continues...
        let active_players_with_stake = new_state
            .players_state
            .iter()
            .copied()
            .filter(|ps| ps.active && ps.stake > 0.0)
            .collect::<Vec<PlayerState>>();

        // If no active players have remaining stake to bet, advance to next stage or end game
        // BUT only if we're not already at Showdown (which should end the game)
        if active_players_with_stake.is_empty() && new_state.stage != Stage::Showdown {
            // All active players are all-in, should advance the game
            match new_state.stage {
                Stage::Preflop => {
                    new_state.stage = Stage::Flop;
                    new_state.current_player = 0;
                }
                Stage::Flop => {
                    new_state.stage = Stage::Turn;
                    new_state.current_player = 0;
                }
                Stage::Turn => {
                    new_state.stage = Stage::River;
                    new_state.current_player = 0;
                }
                Stage::River => {
                    new_state.stage = Stage::Showdown;
                }
                Stage::Showdown => {
                    // Already at showdown, game should end - trigger winner selection
                    new_state.final_state = true;
                }
            }
            // Return immediately since no more actions needed
            return new_state;
        }

        // CRITICAL: If we're at Showdown stage, no more actions are needed - just return
        if new_state.stage == Stage::Showdown {
            new_state.final_state = true;
            return new_state;
        }

        // Move to next player, but skip players who are inactive OR all-in (no remaining stake)
        // BUT don't do this if we're at Showdown - just return
        if new_state.stage == Stage::Showdown {
            return new_state;
        }

        new_state.current_player = (self.current_player + 1) % self.players_state.len() as u64;
        let mut attempts = 0;
        while attempts < self.players_state.len() {
            let current_ps = &new_state.players_state[new_state.current_player as usize];

            // Player is eligible to act if they are active AND have remaining stake to bet
            if current_ps.active && current_ps.stake > 0.0 {
                break;
            }

            new_state.current_player =
                (new_state.current_player + 1) % self.players_state.len() as u64;
            attempts += 1;
        }

        // If we couldn't find any player with chips, all active players are all-in
        if attempts >= self.players_state.len() {
            // All active players are all-in, skip directly to showdown and end game
            while new_state.stage != Stage::Showdown {
                new_state.to_next_stage();
            }

            // Determine winners and end the game immediately
            let active_players: Vec<PlayerState> = new_state
                .players_state
                .iter()
                .copied()
                .filter(|ps| ps.active)
                .collect();

            let ranks: Vec<(u64, u64, u64)> = active_players
                .iter()
                .map(|ps| rank_hand(&new_state, ps.hand, &new_state.public_cards))
                .collect();
            let min_rank = ranks.iter().copied().min().unwrap();

            let winners_indices: Vec<usize> = ranks
                .iter()
                .enumerate()
                .filter(|(_, &r)| r == min_rank)
                .map(|(i, _)| i)
                .collect();

            new_state.set_winners(
                winners_indices
                    .iter()
                    .map(|&i| active_players[i].player)
                    .collect(),
            );

            return new_state;
        }

        // The betting round ends if there are multiple active players
        let active_players: Vec<PlayerState> = new_state
            .players_state
            .iter()
            .copied()
            .filter(|ps| ps.active)
            .collect();
        let multiple_active = active_players.len() >= 2;

        // CRITICAL FIX: If all active players are all-in, skip to showdown and end the game immediately
        let all_active_players_allin = active_players.len() > 1
            && active_players
                .iter()
                .all(|ps| ps.stake < MIN_MEANINGFUL_STAKE);

        if all_active_players_allin {
            // Skip to showdown stage
            while new_state.stage != Stage::Showdown {
                new_state.to_next_stage();
            }

            // Now that we're at showdown, determine winners and end the game
            let ranks: Vec<(u64, u64, u64)> = active_players
                .iter()
                .map(|ps| rank_hand(&new_state, ps.hand, &new_state.public_cards))
                .collect();
            let min_rank = ranks.iter().copied().min().unwrap();

            let winners_indices: Vec<usize> = ranks
                .iter()
                .enumerate()
                .filter(|(_, &r)| r == min_rank)
                .map(|(i, _)| i)
                .collect();

            new_state.set_winners(
                winners_indices
                    .iter()
                    .map(|&i| active_players[i].player)
                    .collect(),
            );

            return new_state;
        }

        // Every active player has done an action this stage OR is all-in (no need to act)
        let all_players_acted = active_players
            .iter()
            .all(|ps| ps.last_stage_action.is_some() || ps.stake < MIN_MEANINGFUL_STAKE);

        // Check if betting is complete: all active players have bet the same amount or are all-in
        let max_bet = active_players
            .iter()
            .map(|ps| ps.bet_chips)
            .fold(0.0f64, f64::max);

        let betting_complete = active_players.iter().all(|ps| {
            // Player has matched the max bet OR is effectively all-in (very small remaining stake)
            ps.bet_chips == max_bet || ps.stake < MIN_MEANINGFUL_STAKE
        });

        // Additional check: if it's preflop, make sure we've gone around the table at least once
        // after the big blind (to handle the case where everyone just calls the big blind)
        let preflop_complete = if self.stage == Stage::Preflop {
            // In preflop, we need to make sure the big blind player has had a chance to act
            // (unless they've already acted by raising)
            let bb_position = (self.button + 2) % self.players_state.len() as u64;
            let bb_player = &new_state.players_state[bb_position as usize];
            !bb_player.active || bb_player.last_stage_action.is_some()
        } else {
            true
        };

        let round_ended =
            multiple_active && all_players_acted && betting_complete && preflop_complete;

        if round_ended && new_state.stage != Stage::Showdown {
            new_state.to_next_stage();
        }

        // Check if only one player has remaining chips (others are all-in)
        let players_with_chips = active_players.iter().filter(|ps| ps.stake > 0.0).count();
        let should_go_to_showdown =
            active_players.len() > 1 && players_with_chips <= 1 && all_players_acted;

        // If only one player has chips left, skip to showdown
        if should_go_to_showdown && new_state.stage != Stage::Showdown {
            while new_state.stage != Stage::Showdown {
                new_state.to_next_stage();
            }
        }

        // CRITICAL FIX: If all active players are all-in (no chips left), advance to showdown immediately
        // But only if we're not already at showdown!
        let all_players_allin = active_players.len() > 1 && players_with_chips == 0;
        if all_players_allin && new_state.stage != Stage::Showdown {
            while new_state.stage != Stage::Showdown {
                new_state.to_next_stage();
            }
        }

        // The game ends if every player except one has folded
        if active_players.len() == 1 {
            new_state.set_winners(vec![active_players[0].player]);
        }

        if new_state.stage == Stage::Showdown {
            let ranks: Vec<(u64, u64, u64)> = active_players
                .iter()
                .map(|ps| rank_hand(&new_state, ps.hand, &new_state.public_cards))
                .collect();
            let min_rank = ranks.iter().copied().min().unwrap();

            // Use verbose_println! macro instead of println!
            verbose_println!(&new_state, "Ranks: {:?}", ranks);

            let winners_indices: Vec<usize> = ranks
                .iter()
                .enumerate()
                .filter(|(_, &r)| r == min_rank)
                .map(|(i, _)| i)
                .collect();

            // Use verbose_println! macro instead of println!
            verbose_println!(&new_state, "Winner id: {:?}", winners_indices);

            new_state.set_winners(
                winners_indices
                    .iter()
                    .map(|&i| active_players[i].player)
                    .collect(),
            );
        }

        new_state.legal_actions = legal_actions(&new_state);
        new_state
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
                // Keep last_stage_action for all-in players so they don't need to act again
                last_stage_action: if ps.stake == 0.0 {
                    ps.last_stage_action
                } else {
                    None
                },
                ..*ps
            })
            .collect();

        self.min_bet = 0.0;

        // Check if all active players are all-in after stage transition
        let active_players_with_chips: Vec<_> = self
            .players_state
            .iter()
            .filter(|ps| ps.active && ps.stake > 0.0)
            .collect();

        // If all active players are all-in, skip directly to showdown
        if active_players_with_chips.is_empty() && self.stage != Stage::Showdown {
            self.stage = Stage::Showdown;
            return;
        }

        self.current_player = (self.button + 1) % self.players_state.len() as u64;

        // Skip to next player who is both active AND has chips to bet
        let mut attempts = 0;
        while attempts < self.players_state.len() {
            let current_ps = &self.players_state[self.current_player as usize];
            if current_ps.active && current_ps.stake > 0.0 {
                break;
            }
            self.current_player = (self.current_player + 1) % self.players_state.len() as u64;
            attempts += 1;
        }

        // If we can't find any player with chips, all are all-in - go to showdown
        if attempts >= self.players_state.len() {
            self.stage = Stage::Showdown;
            return;
        }

        // Update range indices after stage change (important for postflop calculation)
        self.update_range_indices();
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
        Stage::Preflop => {}
        _ => (),
    }

    if state.final_state {
        illegal_actions.append(&mut ActionEnum::iter().collect());
    }

    // Check if current player is all-in (no remaining stake)
    let current_player_state = &state.players_state[state.current_player as usize];
    let is_all_in = current_player_state.stake == 0.0;

    // If player is all-in, they can only fold or check-call (if no bet to call)
    if is_all_in {
        if state.min_bet > current_player_state.bet_chips {
            // There's a bet to call but player is all-in, only fold is allowed
            illegal_actions.push(ActionEnum::CheckCall);
            illegal_actions.push(ActionEnum::BetRaise);
        } else {
            // No bet to call or player has already matched, only check-call is allowed
            illegal_actions.push(ActionEnum::BetRaise);
        }
    }

    let legal_actions: Vec<ActionEnum> = ActionEnum::iter()
        .filter(|a| !illegal_actions.contains(a))
        .collect();
    legal_actions
}

// Modified to accept state parameter for verbose control
fn rank_hand(
    _state: &State,
    private_cards: (Card, Card),
    public_cards: &Vec<Card>,
) -> (u64, u64, u64) {
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

mod tests {
    #[cfg(test)]
    use super::*;
    #[cfg(test)]
    use proptest::prelude::*;

    #[cfg(test)]
    proptest! {
        #[test]
        fn from_deck_doesnt_crash(n_players in 0..10000, deck: Vec<Card>, sb in 0.5_f64..100.0_f64, bb_mult in 2..5, stake_mult in 100..1000, actions: Vec<Action>) {
            let initial_state = State::from_deck(n_players as u64, 0, sb, sb * bb_mult as f64, sb * stake_mult as f64, deck, false);
            match initial_state {
                Ok(mut s) => {
                    for a in actions {
                        s = s.apply_action(a);
                    }
                },
                Err(_) => return Ok(())
            };

        }

        #[test]
        fn zero_sum_game(n_players in 2..26, seed: u64, sb in 0.5_f64..100.0_f64, bb_mult in 2..5, stake_mult in 100..1000, actions in prop::collection::vec(Action::arbitrary_with(((), ())).prop_filter("Raise abs amount bellow 1e12",
        |a| a.amount.abs() < 1e12), 1..100)) {
            let initial_state = State::from_seed(n_players as u64, 0, sb, sb * bb_mult as f64, sb * stake_mult as f64, seed, false);
            match initial_state {
                Ok(mut s) => {
                    for a in actions {
                        s = s.apply_action(a);
                        if s.final_state {
                            let sum_rewards = s.players_state.iter().map(|ps| ps.reward).fold(0_f64, |r1, r2| r1 + r2);
                            println!("sum_rewards = {sum_rewards}");
                            prop_assert!(sum_rewards < 1e-12);
                        }
                    }
                },
                Err(err) => {
                    println!("{}", err.msg);
                    prop_assert!(false);
                }
            };
        }

        #[test]
        fn call_and_check_no_legal_at_same_time(n_players in 2..26, sb in 0.5_f64..100.0_f64, bb_mult in 2..5, stake_mult in 100..1000, actions in prop::collection::vec(Action::arbitrary_with(((), ())), 1..100)) {
            let initial_state = State::from_seed(n_players as u64, 0, sb, sb * bb_mult as f64, sb * stake_mult as f64, 1234, false);
            match initial_state {
                Ok(mut s) => {
                    for a in actions {
                        s = s.apply_action(a);
                        if s.final_state {
                        prop_assert!(!(s.legal_actions.contains(&ActionEnum::CheckCall) && s.legal_actions.contains(&ActionEnum::CheckCall)));
                        }
                    }
                },
                Err(err) => {
                    println!("{}", err.msg);
                    prop_assert!(false);
                }
            };
        }

        #[test]
        fn illegal_raise_own_call(n_players in 2..26, sb in 0.5_f64..100.0_f64, bb_mult in 2..5, stake_mult in 100..1000, actions in prop::collection::vec(Action::arbitrary_with(((), ())), 1..100)) {
            let initial_state = State::from_seed(n_players as u64, 0, sb, sb * bb_mult as f64, sb * stake_mult as f64, 1234, false);
            match initial_state {
                Ok(mut s) => {
                    for a in actions {
                        s = s.apply_action(a);
                        let call_done = s.players_state[s.current_player as usize].last_stage_action == Some(ActionEnum::CheckCall);
                        let not_other_raise = !s.players_state.iter().filter(|ps| ps.active).any(|ps| ps.last_stage_action == Some(ActionEnum::BetRaise));
                        if call_done && not_other_raise {
                            prop_assert!(!s.legal_actions.contains(&ActionEnum::BetRaise));
                        }
                    }
                },
                Err(err) => {
                    println!("{}", err.msg);
                    prop_assert!(false);
                }
            };
        }

        #[test]
        fn illegal_call_zero_bets(n_players in 2..26, sb in 0.5_f64..100.0_f64, bb_mult in 2..5, stake_mult in 100..1000, actions in prop::collection::vec(Action::arbitrary_with(((), ())), 1..100)) {
            let initial_state = State::from_seed(n_players as u64, 0, sb, sb * bb_mult as f64, sb * stake_mult as f64, 1234, false);
            match initial_state {
                Ok(mut s) => {
                    for a in actions {
                        s = s.apply_action(a);
                        // CheckCall is always legal so this test doesn't apply anymore
                        prop_assert!(true);
                    }
                },
                Err(err) => {
                    println!("{}", err.msg);
                    prop_assert!(false);
                }
            };
        }

        #[test]
        fn from_action_not_none(n_players in 2..26, sb in 0.5_f64..100.0_f64, bb_mult in 2..5, stake_mult in 100..1000, actions in prop::collection::vec(Action::arbitrary_with(((), ())), 1..100)) {
            let initial_state = State::from_seed(n_players as u64, 0, sb, sb * bb_mult as f64, sb * stake_mult as f64, 1234, false);
            match initial_state {
                Ok(mut s) => {
                    for a in actions {
                        s = s.apply_action(a);
                        prop_assert!(s.from_action != None);
                    }
                },
                Err(err) => {
                    println!("{}", err.msg);
                    prop_assert!(false);
                }
            };
        }
    }
}
