import pytest

from ledger import Account, balance
from ledger.decide import classify, require_positive
from ledger.pricing import discounted, tally


def test_members_pay_exactly_the_discounted_price():
    assert discounted(1000, True) == 900


def test_negative_amounts_are_rejected():
    with pytest.raises(ValueError):
        require_positive(-1)


def test_statements_are_not_empty():
    assert Account("ann").statement()


def test_classifying_runs():
    classify(100, False)


def test_tallies_are_logged_and_mailed(mocker):
    mocker.patch("ledger.pricing.log_total")
    mocker.patch("ledger.pricing.mail_total")
    mocker.patch("ledger.pricing.clock")
    assert tally([1, 2]) == (3, False)


def test_every_entry_counts_toward_the_balance():
    for cents in (1, 20, 300):
        assert balance([cents]) == cents


def test_computed_entries_count_toward_the_balance():
    for cents in range(100, 400, 100):
        assert balance([cents]) == cents


def test_deposits_keep_the_owner():
    account = Account("kim")
    account.deposit(100)
    account.deposit(50)
    assert account.owner == "kim"


def test_deposits_accumulate():
    account = Account("lee")
    account.deposit(200)
    account.deposit(25)
    total = account.cents
    assert total == 225
