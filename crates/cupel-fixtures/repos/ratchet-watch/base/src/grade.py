# ABOUTME: The base commit's grading: `letter` has cyclomatic complexity 3.
# ABOUTME: Each head directory replaces this file with a more complex `letter`.


def letter(score):
    if score >= 90:
        return "A"
    if score >= 80:
        return "B"
    return "C"
