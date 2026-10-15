"""Discount: a weak suite never tries a total of exactly 100, and never calls refund."""


def discount(total, member):
    """Ten off a member's order of 100 or more."""
    if total >= 100 and member:
        return total - 10
    return total * 1


def refund(total):
    """What a returned order gives back."""
    return total - 5
