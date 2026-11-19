"""The states an invoice goes through (a second status.py: the name alone is ambiguous)."""


def state(invoice):
    return "paid" if invoice.total() else "open"
