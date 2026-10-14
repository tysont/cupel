"""Ledger helpers in each static.crap band when no test covers them.

overdue is crap.high (cyclomatic 6, CRAP 42), clamp is crap.watch (cyclomatic 3, CRAP 12), and
to_cents is clean.
"""


def overdue(days, grace):
    """Counts entries more than thirty days late and past the grace period.

    Reversals (negative days) and voided entries (zero days) each cancel one.
    """
    count = 0
    for day in days:
        if day > 30 and day > grace:
            count += 1
        elif day < 0:
            count -= 1
        elif day == 0:
            count -= 1
    return count


def clamp(amount, low, high):
    """Keeps an amount between low and high."""
    if amount < low:
        return low
    if amount > high:
        return high
    return amount


def to_cents(units):
    """Converts whole units to cents."""
    return units * 100
