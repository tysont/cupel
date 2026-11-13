# ABOUTME: A test that picks its expected letter by a branch, which is test.logic, beside one whose loop holds
# ABOUTME: its assertion, so each pass checks one row of a table and the loop is not logic (ADR-184).
from grade import grade

CASES = [(95, "A"), (85, "B"), (50, "C")]


def test_grade_picks_the_letter_by_branch():
    score = 85
    if score >= 90:
        expected = "A"
    elif score >= 80:
        expected = "B"
    else:
        expected = "C"
    assert grade(score) == expected


def test_grade_checks_each_row_of_the_table():
    for score, expected in CASES:
        assert grade(score) == expected
