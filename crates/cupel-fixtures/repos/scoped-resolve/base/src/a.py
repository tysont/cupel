# ABOUTME: The base commit's first module: `name_a` has cyclomatic complexity 12, past the limit of 10.
# ABOUTME: The `touched` head adds a function beside it; the `fixed` head replaces it with a simple one.

def name_a(n):
    if n == 1:
        return "one"
    if n == 2:
        return "two"
    if n == 3:
        return "three"
    if n == 4:
        return "four"
    if n == 5:
        return "five"
    if n == 6:
        return "six"
    if n == 7:
        return "seven"
    if n == 8:
        return "eight"
    if n == 9:
        return "nine"
    if n == 10:
        return "ten"
    if n == 11:
        return "eleven"
    return "many"
