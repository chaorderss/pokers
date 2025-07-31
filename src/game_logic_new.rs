// game_logic.rs - Rewritten using State-Machine-Based Architecture
use itertools::Itertools;
use pyo3::exceptions::PyOSError;
use pyo3::prelude::*;
use rand::{seq::SliceRandom, SeedableRng};
use std::collections::HashSet;
use strum::IntoEnumIterator;

use crate::state::action::{Action, ActionEnum, ActionRecord};
use crate::state::card::{Card, CardRank, CardSuit};
use crate::state::stage::Stage;
use crate::state::{PlayerState, State, StateStatus};

// Define a macro for verbose printing controlled by environment variable
macro_rules! verbose_println {
    ($state:expr, $($arg:tt)*) => {
        if $state.verbose {
            println!($($arg)*);
            use std::io::Write;
            let _ = std::io::stdout().flush();
        }
    };
}

#[derive(Debug)]
pub struct InitStateError {
    msg: String,
}

impl std::convert::From<InitStateError> for PyErr {
    fn from(err: InitStateError) -> PyErr {
        PyOSError::new_err(err.msg)
    }
}

/// Pot struct for side pot management
#[derive(Debug, Clone)]
pub struct Pot {
    pub amount: f64,
    pub eligible_players: HashSet<u64>,
}

impl Pot {
    pub fn new() -> Self {
        Pot {
            amount: 0.0,
            eligible_players: HashSet::new(),
        }
    }
}

/// Context for a single betting round
#[derive(Debug, Clone)]
pub struct BettingRoundContext {
    pub amount_to_call: f64,
    pub last_raiser_idx: Option<u64>,
    pub actions_this_round: usize,
    pub players_in_round: usize,
    pub starting_player: u64,
}

impl BettingRoundContext {
    pub fn new(amount_to_call: f64, players_in_round: usize, starting_player: u64) -> Self {
        BettingRoundContext {
            amount_to_call,
            last_raiser_idx: None,
            actions_this_round: 0,
            players_in_round,
            starting_player,
        }
    }
}

/// The State trait defining the contract for all game states
pub trait GameStateInterface {
    fn apply_action(
        self: Box<Self>,
        state: &mut State,
        action: Action,
    ) -> Result<Box<dyn GameStateInterface>, StateStatus>;

    fn get_legal_actions(&self, state: &State) -> Vec<ActionEnum>;
    fn state_name(&self) -> String;
    fn is_final(&self) -> bool;
}

/// State for when we are awaiting a player's action
#[derive(Debug, Clone)]
pub struct AwaitingAction {
    pub player_to_act_idx: u64,
    pub context: BettingRoundContext,
}

impl AwaitingAction {
    pub fn new(player_to_act_idx: u64, context: BettingRoundContext) -> Self {
        AwaitingAction {
            player_to_act_idx,
            context,
        }
    }

    /// Check if the betting round has concluded
    fn is_round_over(&self, state: &State) -> bool {
        let active_players: Vec<&PlayerState> =
            state.players_state.iter().filter(|ps| ps.active).collect();

        // Only one player left - round is over
        if active_players.len() <= 1 {
            return true;
        }

        // Check if all players have acted since the last raise or are all-in
        let all_have_acted_or_allin = active_players
            .iter()
            .all(|ps| ps.last_stage_action.is_some() || ps.stake == 0.0);

        // Check if all bets are equal or players are all-in
        let max_bet = active_players
            .iter()
            .map(|ps| ps.bet_chips)
            .fold(0.0f64, f64::max);
        let all_bets_equal = active_players
            .iter()
            .all(|ps| ps.bet_chips == max_bet || ps.stake == 0.0);

        // Special case for preflop big blind option
        let preflop_complete = if state.stage == Stage::Preflop {
            let bb_position = (state.button + 2) % state.players_state.len() as u64;
            let bb_player = &state.players_state[bb_position as usize];
            !bb_player.active || bb_player.last_stage_action.is_some()
        } else {
            true
        };

        all_have_acted_or_allin && all_bets_equal && preflop_complete
    }

    /// Find the next active player who can act
    fn find_next_active_player(&self, state: &State, current_idx: u64) -> Option<u64> {
        let mut next_player = (current_idx + 1) % state.players_state.len() as u64;
        let mut attempts = 0;
        let max_attempts = state.players_state.len();

        while attempts < max_attempts {
            let player_state = &state.players_state[next_player as usize];

            // Player can act if they are active and have chips to bet
            if player_state.active && player_state.stake > 0.0 {
                return Some(next_player);
            }

            next_player = (next_player + 1) % state.players_state.len() as u64;
            attempts += 1;
        }

        None // No eligible player found
    }

    /// Validate that an action is legal
    fn validate_action(&self, state: &State, action: &Action) -> bool {
        let legal_actions = self.get_legal_actions(state);
        legal_actions.contains(&action.action)
    }

    /// Convert illegal action to legal alternative
    fn make_action_legal(&self, state: &State, action: Action) -> Action {
        if self.validate_action(state, &action) {
            return action;
        }

        let legal_actions = self.get_legal_actions(state);

        match action.action {
            ActionEnum::CheckCall => {
                if legal_actions.contains(&ActionEnum::Fold) {
                    Action::new(ActionEnum::Fold, 0.0)
                } else {
                    action // Keep as is if fold is not legal either
                }
            }
            ActionEnum::BetRaise => {
                if legal_actions.contains(&ActionEnum::CheckCall) {
                    Action::new(ActionEnum::CheckCall, 0.0)
                } else if legal_actions.contains(&ActionEnum::Fold) {
                    Action::new(ActionEnum::Fold, 0.0)
                } else {
                    action
                }
            }
            ActionEnum::Fold => action, // Fold should always be legal
        }
    }
}

impl GameStateInterface for AwaitingAction {
    fn apply_action(
        mut self: Box<Self>,
        state: &mut State,
        action: Action,
    ) -> Result<Box<dyn GameStateInterface>, StateStatus> {
        // Validate the action comes from the correct player
        if state.current_player != self.player_to_act_idx {
            return Err(StateStatus::IllegalAction);
        }

        // Make sure action is legal
        let actual_action = self.make_action_legal(state, action);
        let player_idx = self.player_to_act_idx as usize;
        let mut final_action_for_record = actual_action;

        verbose_println!(
            state,
            "DEBUG: Player {} taking action {:?} with amount {}",
            player_idx,
            actual_action.action,
            actual_action.amount
        );

        // Apply the action's effects
        match actual_action.action {
            ActionEnum::Fold => {
                state.players_state[player_idx].active = false;
                state.players_state[player_idx].pot_chips +=
                    state.players_state[player_idx].bet_chips;
                state.players_state[player_idx].bet_chips = 0.0;
                state.players_state[player_idx].reward =
                    -(state.players_state[player_idx].pot_chips);
            }

            ActionEnum::CheckCall => {
                let max_bet = state
                    .players_state
                    .iter()
                    .filter(|ps| ps.active)
                    .map(|ps| ps.bet_chips)
                    .fold(0.0f64, f64::max);

                let current_player_bet = state.players_state[player_idx].bet_chips;
                let is_check = current_player_bet >= max_bet;

                if !is_check {
                    // Call - match the maximum bet
                    let required_chips = max_bet - current_player_bet;
                    let player_stake = state.players_state[player_idx].stake;

                    let actual_chips = if required_chips > player_stake {
                        // Go all-in if can't match
                        state.players_state[player_idx].stake = 0.0;
                        player_stake
                    } else if player_stake - required_chips < 1.0 {
                        // Go all-in if would leave less than 1 chip
                        state.players_state[player_idx].stake = 0.0;
                        player_stake
                    } else {
                        state.players_state[player_idx].stake -= required_chips;
                        required_chips
                    };

                    state.players_state[player_idx].bet_chips += actual_chips;
                    state.pot += actual_chips;

                    final_action_for_record = Action::new(ActionEnum::CheckCall, actual_chips);
                }
            }

            ActionEnum::BetRaise => {
                let desired_total_bet = actual_action.amount;
                let current_player_bet = state.players_state[player_idx].bet_chips;
                let player_stake = state.players_state[player_idx].stake;

                // Calculate actual bet amount
                let actual_total_bet = if player_stake < state.min_bet || player_stake < 1.0 {
                    // Go all-in if insufficient chips
                    current_player_bet + player_stake
                } else if desired_total_bet < state.min_bet {
                    state.min_bet.max(current_player_bet)
                } else {
                    desired_total_bet
                };

                let additional_chips = (actual_total_bet - current_player_bet).max(0.0);
                let final_additional_chips = additional_chips.min(player_stake);

                state.players_state[player_idx].bet_chips += final_additional_chips;
                state.players_state[player_idx].stake -= final_additional_chips;
                state.pot += final_additional_chips;

                // Update minimum bet if this is a valid raise
                if state.players_state[player_idx].bet_chips > state.min_bet {
                    state.min_bet = state.players_state[player_idx].bet_chips;
                    self.context.last_raiser_idx = Some(self.player_to_act_idx);
                    self.context.actions_this_round = 0; // Reset action count on raise
                }

                final_action_for_record = Action::new(
                    ActionEnum::BetRaise,
                    state.players_state[player_idx].bet_chips,
                );
            }
        }

        // Record the action
        state.players_state[player_idx].last_stage_action = Some(actual_action.action);
        self.context.actions_this_round += 1;

        let action_record = ActionRecord {
            player: self.player_to_act_idx,
            action: final_action_for_record,
            stage: state.stage,
            legal_actions: self.get_legal_actions(state),
        };
        state.from_action = Some(action_record.clone());
        state.action_list.push(action_record);

        // Check if round is over
        if self.is_round_over(state) {
            verbose_println!(state, "DEBUG: Round is over, transitioning to next stage");
            return Ok(Box::new(RoundOver::new()));
        }

        // Find next player
        if let Some(next_player_idx) = self.find_next_active_player(state, self.player_to_act_idx) {
            state.current_player = next_player_idx;
            self.player_to_act_idx = next_player_idx;
            Ok(self)
        } else {
            // No more players can act - round is over
            verbose_println!(state, "DEBUG: No more players can act, round over");
            Ok(Box::new(RoundOver::new()))
        }
    }

    fn get_legal_actions(&self, state: &State) -> Vec<ActionEnum> {
        if state.final_state || state.stage == Stage::Showdown {
            return vec![];
        }

        let player_state = &state.players_state[self.player_to_act_idx as usize];

        // If player is all-in, they cannot act
        if player_state.stake == 0.0 {
            return vec![];
        }

        let mut legal_actions = vec![ActionEnum::Fold];

        // Always allow CheckCall
        legal_actions.push(ActionEnum::CheckCall);

        // Allow BetRaise if player has chips to bet
        if player_state.stake > 0.0 {
            legal_actions.push(ActionEnum::BetRaise);
        }

        legal_actions
    }

    fn state_name(&self) -> String {
        format!("AwaitingAction(Player {})", self.player_to_act_idx)
    }

    fn is_final(&self) -> bool {
        false
    }
}

/// Terminal state for a completed betting round
#[derive(Debug, Clone)]
pub struct RoundOver;

impl RoundOver {
    pub fn new() -> Self {
        RoundOver
    }
}

impl GameStateInterface for RoundOver {
    fn apply_action(
        self: Box<Self>,
        _state: &mut State,
        _action: Action,
    ) -> Result<Box<dyn GameStateInterface>, StateStatus> {
        // No actions allowed when round is over
        Err(StateStatus::IllegalAction)
    }

    fn get_legal_actions(&self, _state: &State) -> Vec<ActionEnum> {
        vec![] // No legal actions in this state
    }

    fn state_name(&self) -> String {
        "RoundOver".to_string()
    }

    fn is_final(&self) -> bool {
        true
    }
}

/// Game over state
#[derive(Debug, Clone)]
pub struct GameOver;

impl GameStateInterface for GameOver {
    fn apply_action(
        self: Box<Self>,
        _state: &mut State,
        _action: Action,
    ) -> Result<Box<dyn GameStateInterface>, StateStatus> {
        Err(StateStatus::IllegalAction)
    }

    fn get_legal_actions(&self, _state: &State) -> Vec<ActionEnum> {
        vec![]
    }

    fn state_name(&self) -> String {
        "GameOver".to_string()
    }

    fn is_final(&self) -> bool {
        true
    }
}

/// Internal FSM state holder
struct StateMachine {
    pub current_state: Box<dyn GameStateInterface>,
}

impl StateMachine {
    pub fn new(initial_state: Box<dyn GameStateInterface>) -> Self {
        StateMachine {
            current_state: initial_state,
        }
    }

    pub fn apply_action(&mut self, state: &mut State, action: Action) -> Result<(), StateStatus> {
        let current_state = std::mem::replace(
            &mut self.current_state,
            Box::new(GameOver) as Box<dyn GameStateInterface>,
        );

        match current_state.apply_action(state, action) {
            Ok(new_state) => {
                self.current_state = new_state;
                Ok(())
            }
            Err(status) => {
                // Restore the old state on error
                self.current_state = current_state;
                Err(status)
            }
        }
    }

    pub fn get_legal_actions(&self, state: &State) -> Vec<ActionEnum> {
        self.current_state.get_legal_actions(state)
    }

    pub fn is_final(&self) -> bool {
        self.current_state.is_final()
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

        State::from_deck(n_players, button, sb, bb, stake, deck, verbose, seed)
    }

    #[staticmethod]
    #[pyo3(signature = (n_players, button, sb, bb, stake, deck, verbose=false, seed=0))]
    pub fn from_deck(
        n_players: u64,
        button: u64,
        sb: f64,
        bb: f64,
        stake: f64,
        mut deck: Vec<Card>,
        verbose: bool,
        seed: u64,
    ) -> Result<State, InitStateError> {
        // Validation
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

        if sb <= 0.0 {
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

        // Create players
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
                range_idx: -1,
                last_stage_action: None,
            };
            players_state.push(p_state);
        }

        players_state.sort_by_key(|ps| ps.player);

        // Find first player to act (UTG)
        let first_player = (button + 3) % n_players;

        // Create betting round context
        let active_players = players_state.iter().filter(|ps| ps.active).count();
        let context = BettingRoundContext::new(bb, active_players, first_player);

        // Create initial FSM state
        let initial_fsm_state = Box::new(AwaitingAction::new(first_player, context));

        let mut state = State {
            current_player: first_player,
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
            sb: sb,
            bb: bb,
            status: StateStatus::Ok,
            verbose: verbose,
            seed: seed,
        };

        // Update range indices for all players
        state.update_range_indices();

        // Set legal actions from FSM
        let fsm = StateMachine::new(initial_fsm_state);
        state.legal_actions = fsm.get_legal_actions(&state);

        Ok(state)
    }

    pub fn apply_action(&self, action: Action) -> State {
        match self.status {
            StateStatus::Ok => (),
            _ => return self.clone(),
        }

        if self.final_state {
            return self.clone();
        }

        let mut new_state = self.clone();

        // Create FSM based on current state
        let fsm_state: Box<dyn GameStateInterface> =
            if new_state.stage == Stage::Showdown || new_state.final_state {
                Box::new(GameOver)
            } else {
                // Determine current betting round context
                let active_players = new_state
                    .players_state
                    .iter()
                    .filter(|ps| ps.active)
                    .count();
                let context = BettingRoundContext::new(
                    new_state.min_bet,
                    active_players,
                    new_state.current_player,
                );
                Box::new(AwaitingAction::new(new_state.current_player, context))
            };

        let mut fsm = StateMachine::new(fsm_state);

        match fsm.apply_action(&mut new_state, action) {
            Ok(()) => {
                // Check if we need to transition to next stage
                if fsm.is_final() && !new_state.final_state {
                    new_state.advance_to_next_stage_or_showdown();
                }

                // Update legal actions
                new_state.legal_actions = fsm.get_legal_actions(&new_state);
                new_state
            }
            Err(status) => {
                new_state.status = status;
                new_state
            }
        }
    }

    pub fn __str__(&self) -> PyResult<String> {
        Ok(format!("{:#?}", self))
    }
}

impl State {
    /// Advance to the next stage or handle showdown
    fn advance_to_next_stage_or_showdown(&mut self) {
        verbose_println!(self, "DEBUG: Advancing from stage {:?}", self.stage);

        // Move all bet_chips to pot_chips
        for player_state in &mut self.players_state {
            player_state.pot_chips += player_state.bet_chips;
            player_state.bet_chips = 0.0;
            player_state.last_stage_action = None; // Reset for new stage
        }

        // Advance stage
        self.stage = match self.stage {
            Stage::Preflop => Stage::Flop,
            Stage::Flop => Stage::Turn,
            Stage::Turn => Stage::River,
            Stage::River => Stage::Showdown,
            Stage::Showdown => {
                self.handle_showdown();
                return;
            }
        };

        // Deal community cards
        let cards_to_deal = match self.stage {
            Stage::Flop => 3,
            Stage::Turn | Stage::River => 1,
            _ => 0,
        };

        for _ in 0..cards_to_deal {
            if !self.deck.is_empty() {
                self.public_cards.push(self.deck.remove(0));
            }
        }

        verbose_println!(
            self,
            "DEBUG: Advanced to {:?}, dealt {} cards",
            self.stage,
            cards_to_deal
        );

        // Reset min_bet for new round
        self.min_bet = 0.0;

        // Check if we should go straight to showdown
        let active_players: Vec<&PlayerState> =
            self.players_state.iter().filter(|ps| ps.active).collect();

        let players_with_chips = active_players.iter().filter(|ps| ps.stake > 0.0).count();

        if active_players.len() <= 1 || players_with_chips <= 1 {
            verbose_println!(
                self,
                "DEBUG: Forcing showdown - insufficient active players with chips"
            );
            self.complete_to_showdown();
            return;
        }

        // Find first player to act (left of button)
        self.current_player = (self.button + 1) % self.players_state.len() as u64;
        let mut attempts = 0;

        while attempts < self.players_state.len() {
            let player_state = &self.players_state[self.current_player as usize];
            if player_state.active && player_state.stake > 0.0 {
                break;
            }

            self.current_player = (self.current_player + 1) % self.players_state.len() as u64;
            attempts += 1;
        }

        if attempts >= self.players_state.len() {
            verbose_println!(self, "DEBUG: No players can act, going to showdown");
            self.complete_to_showdown();
        }
    }

    /// Complete to showdown and handle final outcome
    fn complete_to_showdown(&mut self) {
        verbose_println!(self, "DEBUG: Completing to showdown");

        // Deal remaining community cards if needed
        match self.stage {
            Stage::Preflop => {
                // Deal flop, turn, river
                for _ in 0..5 {
                    if !self.deck.is_empty() {
                        self.public_cards.push(self.deck.remove(0));
                    }
                }
            }
            Stage::Flop => {
                // Deal turn, river
                for _ in 0..2 {
                    if !self.deck.is_empty() {
                        self.public_cards.push(self.deck.remove(0));
                    }
                }
            }
            Stage::Turn => {
                // Deal river
                if !self.deck.is_empty() {
                    self.public_cards.push(self.deck.remove(0));
                }
            }
            _ => {} // Already have all cards
        }

        self.stage = Stage::Showdown;
        self.handle_showdown();
    }

    /// Handle showdown logic
    fn handle_showdown(&mut self) {
        verbose_println!(self, "DEBUG: Handling showdown");

        let active_players: Vec<PlayerState> = self
            .players_state
            .iter()
            .copied()
            .filter(|ps| ps.active)
            .collect();

        if active_players.len() <= 1 {
            // Only one player left - they win everything
            if let Some(winner) = active_players.first() {
                self.set_winners(vec![winner.player]);
            }
        } else {
            // Multiple players - evaluate hands
            let mut player_ranks: Vec<(u64, (u64, u64, u64))> = active_players
                .iter()
                .map(|ps| {
                    let rank = rank_hand(self, ps.hand, &self.public_cards);
                    (ps.player, rank)
                })
                .collect();

            // Sort by best hand (lowest rank)
            player_ranks.sort_by_key(|&(_, rank)| rank);

            // Find all players with the best hand
            let best_rank = player_ranks[0].1;
            let winners: Vec<u64> = player_ranks
                .iter()
                .filter(|(_, rank)| *rank == best_rank)
                .map(|(player, _)| *player)
                .collect();

            self.set_winners(winners);
        }
    }

    /// Set winners and calculate rewards
    fn set_winners(&mut self, winners: Vec<u64>) {
        verbose_println!(self, "DEBUG: Setting winners: {:?}", winners);

        // Move all bet_chips to pot_chips for final calculation
        for p in &mut self.players_state {
            p.pot_chips += p.bet_chips;
            p.bet_chips = 0.0;
        }

        // Calculate and distribute rewards using side pot logic
        resolve_pots(self, &winners);

        // Set all players to inactive
        for p in &mut self.players_state {
            p.active = false;
        }

        self.final_state = true;
    }
}

/// Resolve pots and distribute winnings
pub fn resolve_pots(state: &mut State, winners: &[u64]) {
    // Initialize rewards to zero
    for p in &mut state.players_state {
        p.reward = 0.0;
    }

    let showdown_players: Vec<_> = state
        .players_state
        .iter()
        .filter(|p| p.pot_chips > 0.0)
        .cloned()
        .collect();

    if showdown_players.is_empty() {
        return;
    }

    let mut pot_levels: Vec<f64> = showdown_players.iter().map(|p| p.pot_chips).collect();
    pot_levels.sort_by(|a, b| a.partial_cmp(b).unwrap());
    pot_levels.dedup();

    let mut last_level = 0.0;

    for &level in &pot_levels {
        let pot_slice = level - last_level;
        if pot_slice <= 1e-9 {
            continue;
        }

        let contributors: Vec<u64> = state
            .players_state
            .iter()
            .filter(|p| p.pot_chips >= level)
            .map(|p| p.player)
            .collect();

        let total_pot_for_slice = pot_slice * contributors.len() as f64;

        let eligible_players: Vec<u64> = state
            .players_state
            .iter()
            .filter(|p| p.active && p.pot_chips >= level)
            .map(|p| p.player)
            .collect();

        let mut best_rank = (11, 0, 0);
        let mut pot_winners: Vec<u64> = Vec::new();

        for &player_id in &eligible_players {
            let player_state = &state.players_state[player_id as usize];
            let rank = rank_hand(state, player_state.hand, &state.public_cards);

            if rank < best_rank {
                best_rank = rank;
                pot_winners = vec![player_id];
            } else if rank == best_rank {
                pot_winners.push(player_id);
            }
        }

        if !pot_winners.is_empty() {
            let reward_per_winner = total_pot_for_slice / pot_winners.len() as f64;
            for &winner_id in &pot_winners {
                state.players_state[winner_id as usize].reward += reward_per_winner;
            }
        }

        last_level = level;
    }

    // Finalize rewards by subtracting initial investment
    for p in &mut state.players_state {
        p.reward -= p.pot_chips;
    }
}

/// Generate legal actions for the current state - fallback function
#[pyfunction]
pub fn legal_actions(state: &State) -> Vec<ActionEnum> {
    if state.final_state || state.stage == Stage::Showdown {
        return vec![];
    }

    let current_player_state = &state.players_state[state.current_player as usize];

    // If player is all-in, they cannot act
    if current_player_state.stake == 0.0 {
        return vec![];
    }

    let mut legal_actions = vec![ActionEnum::Fold];

    // Always allow CheckCall
    legal_actions.push(ActionEnum::CheckCall);

    // Allow BetRaise if player has chips to bet
    if current_player_state.stake > 0.0 {
        legal_actions.push(ActionEnum::BetRaise);
    }

    legal_actions
}

fn high_card_value(ranks: &Vec<CardRank>) -> u64 {
    let mut value: u64 = 0;
    for (i, &r) in ranks.iter().sorted().enumerate() {
        value += (13_u64.pow(i as u32)) * (12 - r as u64);
    }
    value
}

fn rank_hand(
    _state: &State,
    private_cards: (Card, Card),
    public_cards: &Vec<Card>,
) -> (u64, u64, u64) {
    let mut cards = public_cards.clone();
    cards.append(&mut vec![private_cards.0, private_cards.1]);

    // Check if we have enough cards for a valid combination
    if cards.len() < 5 {
        // Return worst possible rank if not enough cards
        return (10, 0, 0);
    }

    let min_rank = cards
        .iter()
        .copied()
        .combinations(5)
        .map(|comb| rank_card_combination(comb))
        .min()
        .unwrap_or((10, 0, 0));

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
    // Four of a kind: All four cards of the same rank.
    if rank_duplicates[0].0 == 4 {
        let relevant_ranks = vec![rank_duplicates[0].1];
        return (3, high_card_value(&relevant_ranks), high_card_value(&ranks));
    }
    // Full house: Three of a kind with a pair.
    if rank_duplicates[0].0 == 3 && rank_duplicates[1].0 == 2 {
        let relevant_ranks = vec![rank_duplicates[0].1];
        return (4, high_card_value(&relevant_ranks), high_card_value(&ranks));
    }
    // Flush: Any five cards of the same suit, but not in a sequence.
    if suit_duplicates[0].0 == 5 {
        return (5, high_card_value(&ranks), 0_u64);
    }
    // Straight: Five cards in a sequence, but not of the same suit.
    if ranks_in_sequence {
        return (6, high_card_value(&ranks), 0_u64);
    }
    // Three of a kind: Three cards of the same rank.
    if rank_duplicates[0].0 == 3 {
        let relevant_ranks = vec![rank_duplicates[0].1];
        return (7, high_card_value(&relevant_ranks), high_card_value(&ranks));
    }
    // Two pair: Two different pairs.
    if rank_duplicates[0].0 == 2 && rank_duplicates[1].0 == 2 {
        let relevant_ranks = vec![rank_duplicates[0].1, rank_duplicates[1].1];
        return (8, high_card_value(&relevant_ranks), high_card_value(&ranks));
    }
    // Pair: Two cards of the same rank.
    if rank_duplicates[0].0 == 2 {
        let relevant_ranks = vec![rank_duplicates[0].1];
        return (9, high_card_value(&relevant_ranks), high_card_value(&ranks));
    }

    // High Card: When you haven't made any of the hands above, the highest card plays.
    (10, high_card_value(&ranks), 0_u64)
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
            let initial_state = State::from_deck(n_players as u64, 0, sb, sb * bb_mult as f64, sb * stake_mult as f64, deck, false, 12345);
            match initial_state {
                Ok(mut state) => {
                    for action in actions.iter().take(100) {
                        if state.final_state {
                            break;
                        }
                        state = state.apply_action(*action);
                    }
                }
                Err(_) => {}
            };
        }

        #[test]
        fn zero_sum_game(n_players in 2..26, seed: u64, sb in 0.5_f64..100.0_f64, bb_mult in 2..5, stake_mult in 100..1000, actions in prop::collection::vec(Action::arbitrary_with(((), ())).prop_filter("Raise abs amount bellow 1e12",
        |a| a.amount.abs() < 1e12), 1..100)) {
            let initial_state = State::from_seed(n_players as u64, 0, sb, sb * bb_mult as f64, sb * stake_mult as f64, seed, false);
            match initial_state {
                Ok(mut state) => {
                    for action in actions {
                        if state.final_state {
                            break;
                        }
                        state = state.apply_action(action);
                    }
                    let sum: f64 = state.players_state.iter().map(|ps| ps.reward).sum();
                    prop_assert!((sum).abs() < 1e-9);
                }
                Err(_) => {}
            };
        }
    }
}
