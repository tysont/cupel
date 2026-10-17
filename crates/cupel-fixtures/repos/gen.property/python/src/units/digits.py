def digits(n):
    """The number of decimal digits in `n`. The loop stops at five digits, so six-digit numbers
    are miscounted: the bug the property finds once it runs enough cases."""
    count = 1
    limit = 10
    while n >= limit and count < 5:
        count += 1
        limit *= 10
    return count
