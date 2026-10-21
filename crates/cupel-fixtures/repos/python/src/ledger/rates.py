"""Charges computed from the reference cases' rates."""

from .references import _imported_rate


def doubled(amount):
    return _imported_rate(amount)
