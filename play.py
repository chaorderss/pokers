import pokers as prs


def main():
    n_players = int(input("Number of players: "))
    button = int(input("Button: "))

    s = prs.State(n_players=n_players, button=button, seed=1234)
    print(prs.visualize_trace([s]))

    while not s.final_state:
        a_ind = int(
            input(
                f"Choose action {list(zip(range(len(s.legal_actions)), s.legal_actions))}: "
            )
        )
        a = s.legal_actions[a_ind]
        raised_chips = 0
        if a == prs.ActionEnum.Raise:
            clear_line()
            raised_chips = int(input("Chips: "))

        a = prs.Action(action=a, amount=raised_chips)
        s = s.act(a)
        clear_line()
        print(prs.visualize_state(s))


def clear_line():
    print("\033[A                             \033[A")
    print("".join([" "] * 100))
    print("\033[A                             \033[A")


if __name__ == "__main__":
    main()
