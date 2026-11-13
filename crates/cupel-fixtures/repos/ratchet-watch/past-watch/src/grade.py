# ABOUTME: Eight more branches than the base: `letter` has cyclomatic complexity 11, past
# ABOUTME: static.complexity's limit of 10 and the ratchet's tolerance of the base's 3.


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
    if score >= 77:
        return "C+"
    if score >= 73:
        return "C"
    if score >= 70:
        return "C-"
    if score >= 60:
        return "D"
    return "F"
