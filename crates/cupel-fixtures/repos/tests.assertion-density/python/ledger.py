"""A ledger of signed amounts."""


def balance(entries):
    """The sum of `entries`."""
    return sum(entries)


def overdrawn(entries):
    """Whether `entries` sum below zero."""
    return balance(entries) < 0
