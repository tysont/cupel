"""A small order: no discount; never the boundary of exactly 100."""
from discount import discount


def test_keeps_a_small_order():
    assert discount(50, True) == 50
