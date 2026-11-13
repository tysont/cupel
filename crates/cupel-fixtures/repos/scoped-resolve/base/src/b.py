# ABOUTME: The base commit's second module: `size_b` has cyclomatic complexity 13, past the limit of 10.
# ABOUTME: No head changes it, so a changed-scope run never looks at it.


def size_b(n, unit):
    total = 0
    if n > 0:
        total += unit
    if n > 1:
        total += unit
    if n > 2:
        total += unit
    if n > 3:
        total += unit
    if n > 4:
        total += unit
    if n > 5:
        total += unit
    if n > 6:
        total += unit
    if n > 7:
        total += unit
    if n > 8:
        total += unit
    if n > 9:
        total += unit
    if n > 10:
        total += unit
    if n > 11:
        total += unit
    return total
