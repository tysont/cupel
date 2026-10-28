"""Shipping, which no test imports: coverage.py never loads this file."""


def shipping_cost(weight, express):
    """The cost of shipping weight kilograms."""
    base = 5 + weight * 2
    if express:
        return base * 2
    return base


def free_shipping(total):
    """Whether an order of total ships free."""
    return total >= 50
