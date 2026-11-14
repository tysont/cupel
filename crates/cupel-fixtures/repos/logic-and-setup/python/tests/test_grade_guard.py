# ABOUTME: A test whose one assertion is `pytest.fail` under an `if`, the shape of httpx's
# ABOUTME: test_httpcore_all_exceptions_mapped: the `if` guards the failure call, so it is no logic (V14).
import pytest

from grade import grade


def test_grade_gives_a_letter_to_every_score():
    unlettered = [score for score in (0, 50, 100) if grade(score) not in "ABC"]
    if unlettered:  # pragma: no cover
        pytest.fail(f"Scores without a letter: {unlettered}")
