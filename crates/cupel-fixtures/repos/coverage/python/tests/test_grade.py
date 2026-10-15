"""Tests of grade: total and the passing grades of classify."""
from grade import classify, total


def test_totals_a_list():
    assert total([1, 2, 3]) == 6


def test_classifies_passing_scores():
    assert classify(95) == "A"
    assert classify(60) == "pass"
