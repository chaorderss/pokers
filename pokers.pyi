from typing import Optional
from enum import Enum

# visualization.rs ------------------------------------------------------------
def visualize_state(state: State) -> str: ...
def visualize_trace(trace: list[State]) -> str: ...

# paralell.rs -----------------------------------------------------------------
def parallel_act(states: list[State]) -> list[Optional[State]]: ...

# state.rs --------------------------------------------------------------------

class State:
    current_player: int
    players_state: list[PlayerState]
    public_cards: list[Card]
    stage: Stage
    button: int
    from_action: Optional[ActionRecord]
    legal_actions: list[ActionEnum]
    deck: list[Card]
    pot: int
    min_bet: int
    final_state: bool

    def __new__(cls, n_players: int, button: int, seed: int) -> None: ...
    def act(self, action: Action) -> State: ...
    def __str__(self) -> str: ...

class PlayerState:
    player: int
    hand: tuple[Card, Card]
    bet_chips: int
    pot_chips: int
    stake: int
    reward: float
    active: bool

class ActionRecord:
    player: int
    stage: Stage
    action: Action
    legal_actions: list[ActionEnum]

class ActionEnum(Enum):
    Fold = 0
    Check = 1
    Call = 2
    Raise = 3

class Action:
    action: ActionEnum
    amount: int
    def __new__(cls, action: ActionEnum, amount: int = 0) -> None: ...

class Stage(Enum):
    Preflop = 0
    Flop = 1
    Turn = 2
    River = 3
    Showdown = 4

class Card(Enum):
    CA = 0
    C2 = 1
    C3 = 2
    C4 = 3
    C5 = 4
    C6 = 5
    C7 = 6
    C8 = 7
    C9 = 8
    CT = 9
    CJ = 10
    CQ = 11
    CK = 12
    DA = 13
    D2 = 14
    D3 = 15
    D4 = 16
    D5 = 17
    D6 = 18
    D7 = 19
    D8 = 20
    D9 = 21
    DT = 22
    DJ = 23
    DQ = 24
    DK = 25
    HA = 26
    H2 = 27
    H3 = 28
    H4 = 29
    H5 = 30
    H6 = 31
    H7 = 32
    H8 = 33
    H9 = 34
    HT = 35
    HJ = 36
    HQ = 37
    HK = 38
    SA = 39
    S2 = 40
    S3 = 41
    S4 = 42
    S5 = 43
    S6 = 44
    S7 = 45
    S8 = 46
    S9 = 47
    ST = 48
    SJ = 49
    SQ = 50
    SK = 51
