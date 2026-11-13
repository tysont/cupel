# ABOUTME: A base commit's grading near the watch: `letter` has cyclomatic complexity 9, one
# ABOUTME: below static.complexity's limit of 10; past-watch's 11 crosses it.


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
    if score >= 70:
        return "C"
    return "D"
