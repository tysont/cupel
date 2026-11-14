# ABOUTME: Tests of the discount; nothing tests the surcharge.
# ABOUTME: One test, so the per-test map names one test for pricing.py.
from shop.pricing import discount


def test_a_tenth_off_a_hundred_is_ninety():
    assert discount(100, 10) == 90
