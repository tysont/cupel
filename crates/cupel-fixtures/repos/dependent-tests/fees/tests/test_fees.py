# ABOUTME: The depended-on root's own test: a small order pays the fee, which never reaches the large-order
# ABOUTME: boundary, so a `>=` → `>` mutant of `fee` survives it.
from fees import fee


def test_small_orders_pay_the_fee():
    assert fee(10) == 5
