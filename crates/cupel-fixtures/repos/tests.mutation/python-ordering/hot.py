"""A hot path: a fast test and a slow one both run it, and either kills any change to it."""


def scale(x):
    """Three times x."""
    return x * 3
