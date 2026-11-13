# ABOUTME: The dependent root's tests, which run `fees`' code: a large order ships free (killing the boundary
# ABOUTME: mutant of `fee`), a small order pays the fee, and a free code waives it (covering `waived`).
from checkout import charge


def test_large_orders_ship_free():
    assert charge(100) == 100


def test_small_orders_pay_the_fee():
    assert charge(10) == 15


def test_a_free_code_waives_the_fee():
    assert charge(10, "FREE") == 10
