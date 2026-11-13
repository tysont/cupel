# ABOUTME: Five more branches than the base: `letter` has cyclomatic complexity 8, past the
# ABOUTME: ratchet's tolerance of the base's 3 but within static.complexity's limit of 10.


def letter(score):
    if score >= 97:
        return "A+"
    if score >= 93:
        return "A"
    if score >= 90:
        return "A-"
    if score >= 87:
        return "B+"
    if score >= 83:
        return "B"
    if score >= 80:
        return "B-"
    if score >= 70:
        return "C"
    return "D"
