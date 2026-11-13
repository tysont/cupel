# ABOUTME: Three tests that share four setup statements, one fewer than duplicated setup needs at the
# ABOUTME: defaults, so none of them is test.duplicate-setup.
from cart import Cart


def test_two_lines_total():
    cart = Cart()
    cart.add("apple", 0.5, 4)
    cart.add("pear", 0.75, 2)
    cart.discount = 0.0
    assert cart.total() == 3.5


def test_two_lines_count():
    cart = Cart()
    cart.add("apple", 0.5, 4)
    cart.add("pear", 0.75, 2)
    cart.discount = 0.0
    assert cart.count() == 6


def test_two_lines_names():
    cart = Cart()
    cart.add("apple", 0.5, 4)
    cart.add("pear", 0.75, 2)
    cart.discount = 0.0
    assert cart.names() == ["apple", "pear"]
