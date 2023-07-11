#![allow(unused)]
#[cfg(test)]
use proptest_derive::Arbitrary;
use pyo3::prelude::*;
use strum_macros::EnumIter;

#[pyclass]
#[derive(Debug, Clone, Copy, PartialEq, EnumIter)]
#[repr(u32)]
#[cfg_attr(test, derive(Arbitrary))]
pub enum Stage {
    Preflop = 0,
    Flop = 1,
    Turn = 2,
    River = 3,
    Showdown = 4,
}
