# ABOUTME: A script outside the files the project's coverage configuration includes.
# ABOUTME: No test loads it, and coverage.py, told by the project, never measures it.


def summary(prices):
    total = sum(prices)
    return f"{len(prices)} prices, {total} in all"
