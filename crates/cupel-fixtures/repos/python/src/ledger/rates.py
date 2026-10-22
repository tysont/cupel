"""Charges computed from the reference cases' rates."""

from .references import _imported_rate, exported_rate as tripled


def doubled(amount):
    return _imported_rate(amount)


TRIPLED = tripled
