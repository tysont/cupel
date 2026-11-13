# ABOUTME: The fixed head's first module: `name_a` reads a table instead of branching, so its
# ABOUTME: complexity finding is resolved.

NAMES = ["one", "two", "three", "four", "five", "six", "seven", "eight", "nine", "ten", "eleven"]


def name_a(n):
    return NAMES[n - 1] if 1 <= n <= len(NAMES) else "many"
