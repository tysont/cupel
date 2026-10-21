"""Reference cases for static.dead-code (ADR-135): an exported rate, one only an import reaches,
one only a name computed at run time reaches, and one nothing names."""

import sys


def exported_rate(amount):
    return amount * 3


def _imported_rate(amount):
    return amount + amount


def _tiered_rate(amount):
    if amount > 100:
        return amount // 3
    return amount


def _orphan_rate(amount):
    return amount - amount // 4


def rate_named(kind, amount):
    return getattr(sys.modules[__name__], f"_{kind}_rate")(amount)
