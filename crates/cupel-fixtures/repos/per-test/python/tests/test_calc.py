"""Tests of calc: each test covers one function."""
from calc import add, scale


def test_add():
    assert add(2, 3) == 5


def test_scale():
    assert scale(2, 3) == 6
