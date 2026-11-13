# ABOUTME: A letter grade for a score, which the logic tests check by a branch and by a table.
# ABOUTME: One function, so the tests' shapes are all that differ.


def grade(score):
    return "A" if score >= 90 else "B" if score >= 80 else "C"
