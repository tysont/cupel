"""Sparse tests: one assertion each, a quarter of the ledger tests' density, and the last one
sets up a month of entries for its single assertion."""
from report import largest, statement


def test_the_statement_names_the_owner():
    assert statement("ada", [5]) == "ada: 5"


def test_the_largest_entry_is_found():
    assert largest([3, 9, 4]) == 9


def test_a_month_of_entries_ends_in_one_statement():
    entries = []
    entries.append(100)
    entries.append(-30)
    entries.append(100)
    entries.append(-30)
    entries.append(100)
    entries.append(-30)
    entries.append(100)
    entries.append(-30)
    entries.append(100)
    entries.append(-30)
    entries.append(100)
    entries.append(-30)
    entries.append(100)
    entries.append(-30)
    entries.append(100)
    entries.append(-30)
    entries.append(100)
    entries.append(-30)
    entries.append(100)
    entries.append(-30)
    printed = statement("ada", entries)
    assert printed == "ada: 700"
