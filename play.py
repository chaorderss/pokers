import pokers as pkrs


def main():
    n_players = int(input("Number of players: "))
    button = int(input("Button: "))

    s = pkrs.State.from_seed(
        n_players=n_players, button=button, sb=0.5, bb=1.0, stake=100, seed=1234
    )
    print(pkrs.visualize_trace([s]))

    while not s.final_state:
        a_ind = int(
            input(
                f"Choose action {list(zip(range(len(s.legal_actions)), s.legal_actions))}: "
            )
        )
        a = s.legal_actions[a_ind]
        raised_chips = 0
        if a == pkrs.ActionEnum.Raise:
            clear_line()
            raised_chips = int(input("Chips: "))

        a = pkrs.Action(action=a, amount=raised_chips)
        s = s.apply_action(a)
        clear_line()
        print(pkrs.visualize_state(s))


def clear_line():
    print("\033[A                             \033[A")
    print("".join([" "] * 200))
    print("\033[A                             \033[A")


if __name__ == "__main__":
    main()
