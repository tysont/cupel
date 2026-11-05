"""The fast test of the hot path, which sorts after the slow one by name."""
from hot import scale


def test_scales_quickly():
    assert scale(2) == 6
