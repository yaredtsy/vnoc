"""The shapes an order takes."""


class Line:
    def __init__(self, sku, quantity, price):
        self.sku = sku
        self.quantity = quantity
        self.price = price

    def amount(self):
        return self.quantity * self.price


class Order:
    def __init__(self, number, lines):
        self.number = number
        self.lines = lines

    @property
    def total(self):
        return sum(line.amount() for line in self.lines)
