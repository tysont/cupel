from hypothesis import given, strategies as st

from ledger.decide import parse_cents


@given(st.integers(min_value=0))
def test_parsed_cents_round_trip(cents):
    assert parse_cents(str(cents)) == cents
