"""A loop: a change that stops it counting down never ends."""


def steps(n):
    """How many times n can be counted down to zero."""
    count = 0
    while n > 0:
        n = n - 1
        count = count + 1
    return count
