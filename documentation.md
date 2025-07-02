# Pokers Python Library Documentation

**Embarrassingly simple No Limit Texas Holdem environment for RL**

## Table of Contents

1. [Introduction](#introduction)
2. [Installation](#installation)
3. [Core Concepts](#core-concepts)
4. [Quick Start](#quick-start)
5. [API Reference](#api-reference)
   - [State](#state)
   - [Player State](#player-state)
   - [Actions](#actions)
   - [Cards](#cards)
   - [Game Stages](#game-stages)
   - [Visualization](#visualization)
   - [Parallel Execution](#parallel-execution)
6. [Examples](#examples)
7. [Testing](#testing)

## Introduction

Pokers is a high-performance No-Limit Texas Hold'em poker environment implemented in Rust with Python bindings. It is designed primarily for reinforcement learning applications, providing an efficient and accurate poker game simulation.

The library:
- Implements the full rules of No-Limit Texas Hold'em
- Handles game state, player actions, card evaluation, and winner determination
- Supports visualization of game states and action histories
- Provides efficient parallel execution for batch processing
- Enables seeded game initialization for reproducibility

## Installation

The library can be installed using pip:

```bash
pip install pokers
```

For development:

```bash
pip install pokers[dev]
```

Requirements:
- Python 3.7 or newer
- Rust compiler (for building from source)

## Core Concepts

Pokers implements a Texas Hold'em poker game with the following key elements:

### State and Game Flow

A poker game is represented by a `State` object that keeps track of all game information:
- Player hands, bets, and status
- Community cards 
- Current game stage (preflop, flop, turn, river, showdown)
- Pot size and minimum bet
- Legal actions for the current player
- Button position and current player turn

The game proceeds through stages (preflop → flop → turn → river → showdown) with betting rounds between each stage. The game ends when either:
- Only one player remains active (all others folded)
- The game reaches showdown and a winner is determined by hand evaluation

### Actions

Players can take one of four possible actions on their turn:
- **Fold**: Surrender the hand and forfeit any chips contributed to the pot
- **Check**: Pass the action to the next player without betting (only if no one has bet in the current round)
- **Call**: Match the current bet amount
- **Raise**: Increase the bet amount (requires specifying the raise amount)

### Hand Evaluation

The library implements standard poker hand rankings to determine the winner in a showdown:
1. Royal Flush
2. Straight Flush
3. Four of a Kind
4. Full House
5. Flush
6. Straight
7. Three of a Kind
8. Two Pair
9. One Pair
10. High Card

## Quick Start

Here's a simple example to create and play a poker game:

```python
import pokers as pkrs

# Create a new poker game with 4 players, button at position 0
# Small blind 0.5, big blind 1.0, and starting stake 100
state = pkrs.State.from_seed(
    n_players=4, 
    button=0, 
    sb=0.5, 
    bb=1.0, 
    stake=100, 
    seed=42
)

# Display the initial game state
print(pkrs.visualize_state(state))

# Player takes an action (e.g., call)
action = pkrs.Action(action=pkrs.ActionEnum.Call)
new_state = state.apply_action(action)

# Player raises
raise_action = pkrs.Action(action=pkrs.ActionEnum.Raise, amount=2.0)
new_state = new_state.apply_action(raise_action)

# Run until game completion
while not new_state.final_state:
    # Get legal actions for current player
    legal_actions = new_state.legal_actions
    
    # Choose an action (for example, always call if possible)
    if pkrs.ActionEnum.Call in legal_actions:
        action = pkrs.Action(action=pkrs.ActionEnum.Call)
    else:
        action = pkrs.Action(action=pkrs.ActionEnum.Check)
    
    # Apply the action
    new_state = new_state.apply_action(action)
    
    # Visualize the current state
    print(pkrs.visualize_state(new_state))

# Check rewards to see who won
for player in new_state.players_state:
    print(f"Player {player.player}: {player.reward}")
```

## API Reference

### State

The `State` class represents the complete game state.

#### Attributes

- `current_player: int` - Index of the player whose turn it is
- `players_state: List[PlayerState]` - List of player states
- `public_cards: List[Card]` - Community cards on the board
- `stage: Stage` - Current game stage (preflop, flop, turn, river, showdown)
- `button: int` - Position of the dealer button
- `from_action: Optional[ActionRecord]` - Record of the last action taken
- `legal_actions: List[ActionEnum]` - Valid actions for the current player
- `deck: List[Card]` - Remaining cards in the deck
- `pot: float` - Total pot size
- `min_bet: float` - Current minimum bet amount
- `final_state: bool` - Whether the game has ended
- `status: StateStatus` - Game status (Ok, IllegalAction, HighBet)

#### Methods

**Static Constructors**

```python
@staticmethod
def from_seed(n_players: int, button: int, sb: float, bb: float, stake: float, seed: int) -> State
```
Creates a new game state with randomly shuffled cards using the provided seed.

- `n_players`: Number of players (minimum 2)
- `button`: Position of the dealer button (0 to n_players-1)
- `sb`: Small blind amount
- `bb`: Big blind amount
- `stake`: Starting chips for each player
- `seed`: Random seed for reproducibility

```python
@staticmethod
def from_deck(n_players: int, button: int, sb: float, bb: float, stake: float, deck: List[Card]) -> State
```
Creates a new game state with a predefined deck of cards.

**Game Progression**

```python
def apply_action(self, action: Action) -> State
```
Applies the given action to the current state and returns the new state.

### Player State

The `PlayerState` class represents an individual player's state in the game.

#### Attributes

- `player: int` - Player index
- `hand: Tuple[Card, Card]` - Player's hole cards
- `bet_chips: float` - Chips bet in the current round
- `pot_chips: float` - Chips committed to the pot from previous rounds
- `stake: float` - Remaining chips available to bet
- `reward: float` - Player's reward (positive if won, negative if lost)
- `active: bool` - Whether the player is still active in the hand
- `last_stage_action` - Player's last action in the current stage

### Actions

Actions represent the moves players can make during the game.

#### ActionEnum

Enumeration of possible actions:
- `Fold` - Give up the hand
- `Check` - Pass without betting
- `Call` - Match the current bet
- `Raise` - Increase the bet

#### Action Class

```python
class Action:
    action: ActionEnum
    amount: float
    
    def __new__(cls, action: ActionEnum, amount: float = 0) -> None
```

- `action`: The type of action (Fold, Check, Call, Raise)
- `amount`: The amount to raise by (only used for Raise actions)

#### ActionRecord

Records an action taken by a player:

```python
class ActionRecord:
    player: int
    stage: Stage
    action: Action
    legal_actions: List[ActionEnum]
```

### Cards

Cards are represented by suit and rank.

#### Card Class

```python
class Card:
    suit: CardSuit
    rank: CardRank
    
    @staticmethod
    def from_string(string: str) -> Optional[Card]
    
    @staticmethod
    def collect() -> List[Card]
```

- `from_string`: Creates a card from a string representation (e.g., "C2" for 2 of Clubs)
- `collect`: Creates a standard 52-card deck

#### CardSuit

Enumeration of card suits:
- `Clubs`
- `Diamonds`
- `Hearts`
- `Spades`

#### CardRank

Enumeration of card ranks:
- `R2` to `R9` - Number cards 2-9
- `RT` - Ten
- `RJ` - Jack
- `RQ` - Queen
- `RK` - King
- `RA` - Ace

### Game Stages

```python
class Stage(Enum):
    Preflop = 0   # Initial betting round with hole cards only
    Flop = 1      # First three community cards dealt
    Turn = 2      # Fourth community card dealt
    River = 3     # Fifth community card dealt
    Showdown = 4  # Final hand evaluation
```

### Visualization

```python
def visualize_state(state: State) -> str
```
Returns a string representation of the current game state, showing player bets, community cards, and pot size.

```python
def visualize_trace(trace: List[State]) -> str
```
Visualizes a sequence of states, showing the progression of a game.

### Parallel Execution

```python
def parallel_apply_action(states: List[State], actions: List[Action]) -> List[State]
```
Applies multiple actions to multiple states in parallel, useful for batch processing in reinforcement learning applications.

### State Status

```python
class StateStatus(Enum):
    Ok = 0              # Normal state
    IllegalAction = 1   # An illegal action was attempted
    HighBet = 2         # A bet exceeds available chips
```

## Examples

### Complete Game Example

```python
import pokers as pkrs
import random

# Initialize a game with 6 players
state = pkrs.State.from_seed(
    n_players=6,
    button=0,
    sb=0.5,
    bb=1.0,
    stake=100.0,
    seed=42
)

# Print the initial state
print("Initial state:")
print(pkrs.visualize_state(state))

# Play the game until completion
states = [state]
while not state.final_state:
    # Get legal actions for current player
    legal_actions = state.legal_actions
    
    # Choose a random legal action
    action_type = random.choice(legal_actions)
    
    # If the action is Raise, add a random amount
    amount = 0.0
    if action_type == pkrs.ActionEnum.Raise:
        amount = random.uniform(2.0, 10.0)
    
    action = pkrs.Action(action=action_type, amount=amount)
    
    # Apply the action
    state = state.apply_action(action)
    states.append(state)
    
    print(f"Player {state.from_action.player} performed {action.action}")
    print(pkrs.visualize_state(state))

# Print game trace
print("\nComplete game history:")
print(pkrs.visualize_trace(states))

# Print results
print("\nFinal results:")
for player in state.players_state:
    result = "won" if player.reward > 0 else "lost"
    print(f"Player {player.player} {result} {abs(player.reward):.2f} chips")
```

### Custom Card Deck

```python
import pokers as pkrs

# Create a custom deck of cards
cards = []
for suit in [pkrs.CardSuit.Hearts, pkrs.CardSuit.Spades, 
             pkrs.CardSuit.Diamonds, pkrs.CardSuit.Clubs]:
    for rank in [pkrs.CardRank.R2, pkrs.CardRank.R3, pkrs.CardRank.R4,
                 pkrs.CardRank.R5, pkrs.CardRank.R6, pkrs.CardRank.R7,
                 pkrs.CardRank.R8, pkrs.CardRank.R9, pkrs.CardRank.RT,
                 pkrs.CardRank.RJ, pkrs.CardRank.RQ, pkrs.CardRank.RK,
                 pkrs.CardRank.RA]:
        cards.append(pkrs.Card(suit=suit, rank=rank))

# Arrange the deck to create specific hands for testing
# ... (custom arrangement)

# Initialize the game with the custom deck
state = pkrs.State.from_deck(
    n_players=2,
    button=0,
    sb=0.5,
    bb=1.0,
    stake=100.0,
    deck=cards
)
```

### Parallel Evaluation for Reinforcement Learning

```python
import pokers as pkrs
import random

# Create multiple game states
states = []
for i in range(16):
    state = pkrs.State.from_seed(
        n_players=2,
        button=0,
        sb=0.5,
        bb=1.0,
        stake=100.0,
        seed=i
    )
    states.append(state)

# Define random actions for each state
actions = []
for state in states:
    action_type = random.choice(state.legal_actions)
    amount = 0.0
    if action_type == pkrs.ActionEnum.Raise:
        amount = random.uniform(2.0, 10.0)
    actions.append(pkrs.Action(action=action_type, amount=amount))

# Apply actions in parallel
new_states = pkrs.parallel_apply_action(states, actions)

# Process results
for i, state in enumerate(new_states):
    print(f"State {i}: Player {state.from_action.player} performed {actions[i].action}")
```

## Testing

The library includes property-based testing to ensure game mechanics work correctly:

- Ensuring the game is zero-sum (rewards sum to zero)
- Verifying that call and check are not both legal at the same time
- Testing that raises work properly
- Verifying that invalid actions are rejected

Run tests with:

```bash
pytest
```

For development:

```bash
# Formatting
black .

# Type checking
mypy .

# Linting
ruff check .
```