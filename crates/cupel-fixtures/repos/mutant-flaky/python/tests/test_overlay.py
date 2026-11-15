"""Fails wherever Cupel runs mutants: every mutation run sets CUPEL_MUTANT, even with no mutant selected."""
import os

from calc import add, is_positive


def test_outside_mutation_slots():
    assert add(1, 1) == 2
    assert is_positive(1)
    assert "CUPEL_MUTANT" not in os.environ
