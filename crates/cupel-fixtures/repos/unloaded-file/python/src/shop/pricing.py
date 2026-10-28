"""Prices, fully covered by the tests."""


def price(unit, quantity):
    """The price of quantity units."""
    return unit * quantity


def discounted(total, percent):
    """The total less a percentage."""
    return total - total * percent / 100
