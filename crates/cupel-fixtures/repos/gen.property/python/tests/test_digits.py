from hypothesis import given, strategies as st

from units.digits import digits


@given(st.integers(min_value=0, max_value=109_999))
def test_digits_counts_decimal_places(n):
    assert digits(n) == len(str(n))
