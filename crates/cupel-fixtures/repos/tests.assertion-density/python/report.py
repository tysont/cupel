"""Reports drawn from a ledger."""
from ledger import balance


def statement(owner, entries):
    """One line naming `owner` and the balance of `entries`."""
    return f"{owner}: {balance(entries)}"


def largest(entries):
    """The largest of `entries`, or None."""
    return max(entries, default=None)
