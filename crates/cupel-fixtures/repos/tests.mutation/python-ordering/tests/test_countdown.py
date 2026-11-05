"""The countdown's test, which finishes at once unless its loop never ends."""
from countdown import steps


def test_counts_down_three_steps():
    assert steps(3) == 3
