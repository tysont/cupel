"""Adding and the sign test: `is_positive` is checked only far from its boundary."""
from calc import add, is_positive


def test_adds():
    assert add(2, 3) == 5


def test_five_is_positive():
    assert is_positive(5)
