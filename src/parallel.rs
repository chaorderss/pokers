use crate::state::action::Action;
use crate::state::State;
use pyo3::prelude::*;
use rayon::prelude::*;

#[pyfunction]
pub fn parallel_apply_action(states: Vec<State>, actions: Vec<Action>) -> Vec<State> {
    states
        .par_iter()
        .zip(actions)
        .map(|(s, a)| s.apply_action(a))
        .collect()
}
