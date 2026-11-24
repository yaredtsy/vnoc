# FileID: 4Q57CGHF0HJ23TTYWXTZXQ8DPS
"""Charging a customer's card through the payment gateway."""

MAX_ATTEMPTS = 3


class ChargeError(Exception):
    """The gateway refused the charge."""


def retry_charge(gateway, order, attempts=MAX_ATTEMPTS):
    """Charge the order, trying again when the gateway drops the call.

    ID: 7RGEHDGW99TJVQE9NG5D8WXZJN
    """

    def one_try():
        return gateway.charge(order.total)

    for _ in range(attempts):
        try:
            return one_try()
        except ConnectionError:
            continue
    raise ChargeError(f"order {order.number}: the gateway kept dropping the call")


def refund(gateway, order):
    """Give the money back, in full."""
    return gateway.refund(order.total)
