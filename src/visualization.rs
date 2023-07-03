use crate::state::State;
use pyo3::prelude::*;

#[pyfunction]
pub fn visualize_trace(trace: Vec<State>) -> String {
    let players = trace[0]
        .players_state
        .iter()
        .map(|ps| {
            ps.player.to_string()
                + if trace[0].button == ps.player {
                    "·"
                } else {
                    ""
                }
        })
        .fold("       ".to_owned(), |s1, s2| format!("{s1}       {s2:<3}"));

    let hands = trace[0]
        .players_state
        .iter()
        .map(|ps| format!("|{0} {1}|", ps.hand.0, ps.hand.1))
        .fold("        ".to_owned(), |s1, s2| format!("{s1}   {s2}"));

    let vis = trace
        .iter()
        .map(|state| visualize_state(state))
        .fold(format!("{players}    pot    public\n{hands}"), |s1, s2| {
            format!("{s1}\n{s2}")
        });

    vis
}

#[pyfunction]
pub fn visualize_state(state: &State) -> String {
    let action = match &state.from_action {
        None => "".to_owned(),
        Some(action_record) => {
            let action_offset = 14 + 10 * action_record.player;
            let pad = std::iter::repeat(" ")
                .take(action_offset as usize)
                .collect::<String>();
            if action_record.action.amount == 0.0 {
                format!("{pad}↓ {:?}\n", action_record.action.action)
            } else {
                format!(
                    "{pad}↓ {:?}({})\n",
                    action_record.action.action, action_record.action.amount
                )
            }
        }
    };

    let players_bets = state
        .players_state
        .iter()
        .map(|ps| {
            format!(
                "{0:>4}/{1:<3}",
                if ps.active {
                    ps.bet_chips.to_string()
                } else {
                    format!("x{}", ps.bet_chips)
                },
                ps.bet_chips + ps.stake,
            )
        })
        .fold("".to_owned(), |s1, s2| format!("{s1}  {s2}"));

    let public_cards = state
        .public_cards
        .iter()
        .fold("".to_owned(), |c1, c2| format!("{0} {1}", c1, c2));
    format!(
        "{action}{0:<9?}:{players_bets}  {1:>4}    |{public_cards}|",
        state.stage, state.pot
    )
}
