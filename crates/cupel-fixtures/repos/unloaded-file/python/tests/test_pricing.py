"""Tests of pricing alone; nothing here imports shipping."""
from shop.pricing import discounted, price


def test_prices_a_quantity():
    assert price(3, 4) == 12


def test_discounts_a_total():
    assert discounted(200, 10) == 180
