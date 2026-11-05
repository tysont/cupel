"""The slow test of the hot path: it kills every mutant too, but only after six seconds."""
import time

from hot import scale


def test_scales_slowly():
    time.sleep(6)
    assert scale(5) == 15
