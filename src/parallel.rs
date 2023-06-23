use crate::game_logic::ActionError;
use crate::state::{Action, State};
use pyo3::prelude::*;
use rayon::prelude::*;

fn parallel_apply_action(
    states: Vec<State>,
    actions: Vec<Action>,
) -> Vec<Result<State, ActionError>> {
    states
        .par_iter()
        .zip(actions)
        .map(|(s, a)| s.apply_action(a))
        .collect()
}
