"""Dense tests: each pins every outcome it exercises."""
from ledger import balance, overdrawn


def test_deposits_and_withdrawals_add_up():
    assert balance([]) == 0
    assert balance([5]) == 5
    assert balance([5, -3]) == 2
    assert balance([-3, -4]) == -7


def test_a_negative_balance_is_overdrawn():
    assert overdrawn([-1])
    assert overdrawn([5, -6])
    assert not overdrawn([])
    assert not overdrawn([5, -5])


def test_order_does_not_change_the_balance():
    assert balance([1, 2, 3]) == 6
    assert balance([3, 2, 1]) == 6
    assert balance([2, -2, 1]) == 1
    assert balance([-2, 1, 2]) == 1
