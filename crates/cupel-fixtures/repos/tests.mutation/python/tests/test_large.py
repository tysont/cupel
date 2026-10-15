"""A large member order: the discount applies."""
from discount import discount


def test_discounts_a_large_member_order():
    assert discount(200, True) == 190
