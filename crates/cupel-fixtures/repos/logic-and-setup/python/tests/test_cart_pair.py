# ABOUTME: Two tests that share five setup statements, fewer than the three tests duplicated setup needs
# ABOUTME: at the defaults, so neither is test.duplicate-setup.
from cart import Cart


def test_a_discount_lowers_the_total():
    cart = Cart()
    cart.add("apple", 0.5, 4)
    cart.add("pear", 0.75, 2)
    cart.discount = 0.5
    total = cart.total()
    assert total == 1.75


def test_no_discount_keeps_the_total():
    cart = Cart()
    cart.add("apple", 0.5, 4)
    cart.add("pear", 0.75, 2)
    cart.discount = 0.0
    total = cart.total()
    assert total == 3.5
