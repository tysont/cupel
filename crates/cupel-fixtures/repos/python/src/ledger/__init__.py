"""A small ledger: accounts, pricing, and reports."""

__all__ = ["Account", "Audited", "balance", "MAX_ENTRIES", "default_currency"]

from .account import Account, Audited
from .generated import schema

MAX_ENTRIES: int = 1000
default_currency = "USD"
DEBUG = False


def balance(entries: list[int]) -> int:
    return sum(entries)


def audit_limit(entries):
    return _scale(len(entries))


def _scale(count):
    return count * 2
