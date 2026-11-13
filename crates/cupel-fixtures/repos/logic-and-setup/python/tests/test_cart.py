# ABOUTME: Three tests that arrange the same five statements before asserting, which is test.duplicate-setup
# ABOUTME: at the defaults (three tests, five statements); the finding is about the first of them.
from cart import Cart


def test_total_sums_every_line():
    cart = Cart()
    cart.add("apple", 0.5, 4)
    cart.add("pear", 0.75, 2)
    cart.add("plum", 0.25, 8)
    cart.discount = 0.1
    assert cart.total() == 4.95


def test_count_sums_every_quantity():
    cart = Cart()
    cart.add("apple", 0.5, 4)
    cart.add("pear", 0.75, 2)
    cart.add("plum", 0.25, 8)
    cart.discount = 0.1
    assert cart.count() == 14


def test_names_keep_the_order_added():
    cart = Cart()
    cart.add("apple", 0.5, 4)
    cart.add("pear", 0.75, 2)
    cart.add("plum", 0.25, 8)
    cart.discount = 0.1
    assert cart.names() == ["apple", "pear", "plum"]
