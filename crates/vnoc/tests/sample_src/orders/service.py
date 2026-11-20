"""Placing and cancelling orders."""

from orders.models import Line, Order


class OrderService:
    def __init__(self, store):
        self.store = store

    def parse_order(self, raw):
        lines = [Line(item["sku"], item["qty"], item["price"]) for item in raw["items"]]
        return Order(raw["number"], lines)

    def place(self, raw):
        order = self.parse_order(raw)
        self.store.save(order)
        return order

    def cancel(self, number):
        order = self.store.load(number)
        self.store.delete(number)
        return order
