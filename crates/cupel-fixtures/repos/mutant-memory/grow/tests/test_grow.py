"""Growing to three chunks: the loop's step decides when it ends."""
from grow import grow


def test_grows_to_three():
    assert grow(3) == 3
