# ABOUTME: The other side of a cycle of imports that both run.
# ABOUTME: It imports the left side's module.
from . import _left

RIGHT = 1


def both() -> int:
    return _left.LEFT + RIGHT
