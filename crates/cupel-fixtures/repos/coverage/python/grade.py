"""Grade: a suite that covers total fully, classify partly, and refund not at all."""


def total(values):
    """The sum of values."""
    result = 0
    for value in values:
        result += value
    return result


def classify(score):
    """A letter for score; the tests never reach the failing grade."""
    if score >= 90:
        return "A"
    if score >= 50:
        return "pass"
    return "fail"


def refund(amount, days, member):
    """What to refund of amount after days; no test calls it."""
    if amount <= 0:
        return 0
    if days > 30:
        return 0
    if member and days < 7:
        return amount
    if member:
        return amount / 2
    return amount / 4
