import pokers as pkrs
import json
from itertools import zip_longest


def test_parallel_apply_action_against_pluribus_logs():
    with open("tests/test_files/pluribus_logs.json") as f:
        pluribus_data = json.load(f)

    batch_size = 100

    for i in range(0, len(pluribus_data), batch_size):
        print(i)
        pb_hand_batch = pluribus_data[i : i + batch_size]
        pkrs_states = []
        for pb_hand in pb_hand_batch:
            n_players = len(pb_hand["players"])
            button = pb_hand["button"]
            str_deck = [c for hand in pb_hand["private_cards"] for c in hand]
            if "public_cards" in pb_hand:
                if "flop" in pb_hand["public_cards"]:
                    str_deck += pb_hand["public_cards"]["flop"]
                if "turn" in pb_hand["public_cards"]:
                    str_deck += [pb_hand["public_cards"]["turn"]]
                if "river" in pb_hand["public_cards"]:
                    str_deck += [pb_hand["public_cards"]["river"]]
            deck = []
            for str_c in str_deck:
                c = pkrs.Card.from_string(str_c[::-1])
                assert c is not None
                deck.append(c)

            pkrs_state = pkrs.State.from_deck(
                n_players=n_players,
                button=button,
                deck=deck,
                sb=50,
                bb=100,
                stake=float("inf"),
            )
            pkrs_states.append(pkrs_state)

        actions_batch = zip_longest(
            *[
                [a for actions in pb_hand["actions"].values() for a in actions]
                for pb_hand in pb_hand_batch
            ],
            fillvalue=({"player": 0, "action": "fold"}),
        )

        stages_batch = zip_longest(
            *[
                [
                    stage
                    for (stage, actions) in pb_hand["actions"].items()
                    for _ in actions
                ]
                for pb_hand in pb_hand_batch
            ],
            fillvalue=("showdown"),
        )

        for pb_actions, pb_stages in zip(actions_batch, stages_batch):
            actions = [
                pkrs.Action(
                    pkrs.ActionEnum.__dict__[a["action"].capitalize()],
                    a.get("amount", 0),
                )
                for a in pb_actions
            ]
            for pkrs_state, pb_action, pb_stage in zip(
                pkrs_states, pb_actions, pb_stages
            ):
                if not pkrs_state.final_state:
                    assert (
                        pkrs_state.stage == pkrs.Stage.__dict__[pb_stage.capitalize()]
                    )
                    assert pkrs_state.current_player == pb_action["player"]
                    assert pkrs_state.status == pkrs.StateStatus.Ok
            pkrs_states = pkrs.parallel_apply_action(pkrs_states, actions)

        for pkrs_state, pb_hand in zip(pkrs_states, pb_hand_batch):
            assert pkrs_state.status == pkrs.StateStatus.Ok
            assert pkrs_state.final_state
            for p, r in enumerate(pb_hand["rewards"]):
                assert pkrs_state.players_state[p].reward == r
