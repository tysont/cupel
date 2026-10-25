# ABOUTME: The one importer of `price`, which the head commit leaves unchanged.
# ABOUTME: Its edge is the only thing that keeps `shop.prices` live.
from shop.prices import price


def total(items):
    return sum(price(item) for item in items)
