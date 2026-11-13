# ABOUTME: The dependent root's code, which imports the `fees` root (`pythonpath` in its pyproject):
# ABOUTME: a charge is the total plus its fee, unless a code waives it.
from fees import fee, waived


def charge(total, code=""):
    if waived(code):
        return total
    return total + fee(total)
