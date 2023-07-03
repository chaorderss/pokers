use pyo3::prelude::*;
use strum_macros::EnumIter;

#[pyclass]
#[derive(Debug, Clone, Copy, PartialEq, EnumIter)]
pub enum Stage {
    Preflop,
    Flop,
    Turn,
    River,
    Showdown,
}
