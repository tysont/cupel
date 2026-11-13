# ABOUTME: One more `if` than the base: `letter` has cyclomatic complexity 4, within
# ABOUTME: the ratchet's tolerance of the base's 3.


def letter(score):
    if score >= 90:
        return "A"
    if score >= 80:
        return "B"
    if score >= 70:
        return "C"
    return "D"
