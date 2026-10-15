from tally import total


def test_totals_a_list():
    assert total([1, 2, 3]) == 6
