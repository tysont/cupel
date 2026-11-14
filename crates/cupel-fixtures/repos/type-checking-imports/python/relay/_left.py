# ABOUTME: One side of a cycle of imports that both run.
# ABOUTME: It imports the right side's constant.
from ._right import RIGHT

LEFT = RIGHT + 1
