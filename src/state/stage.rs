#![allow(unused)]
#[cfg(test)]
use proptest_derive::Arbitrary;
use pyo3::prelude::*;
use strum_macros::EnumIter;

#[pyclass]
#[derive(Debug, Clone, Copy, PartialEq, EnumIter)]
#[cfg_attr(test, derive(Arbitrary))]
pub enum Stage {
    Preflop,
    Flop,
    Turn,
    River,
    Showdown,
}
