"""Sales tax by country."""

RATES = {"DE": 0.19, "FR": 0.20, "US": 0.0}


def rate_for(country):
    return RATES.get(country, 0.0)


def apply_tax(amount, country):
    return round(amount * (1 + rate_for(country)), 2)
