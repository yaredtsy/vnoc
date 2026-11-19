"""The states an order goes through."""

STATES = ("new", "paid", "shipped", "cancelled")


def next_state(state):
    index = STATES.index(state)
    return STATES[min(index + 1, len(STATES) - 2)]
