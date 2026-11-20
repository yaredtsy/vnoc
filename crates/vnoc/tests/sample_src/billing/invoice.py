"""Invoices: what the customer is shown after a charge."""

from billing.tax import apply_tax


class Invoice:
    """The lines of one order, with the tax worked out."""

    def __init__(self, country):
        self.country = country
        self.lines = []

    def add_line(self, text, amount):
        self.lines.append((text, amount))

    def total(self):
        net = sum(amount for _, amount in self.lines)
        return apply_tax(net, self.country)


def render_invoice(invoice):
    rows = [f"{text:<30}{amount:>10.2f}" for text, amount in invoice.lines]
    rows.append(f"{'Total':<30}{invoice.total():>10.2f}")
    return "\n".join(rows)
