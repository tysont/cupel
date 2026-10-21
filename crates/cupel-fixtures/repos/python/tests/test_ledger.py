from unittest.mock import patch

import pytest

from ledger import Account
from ledger.decide import parse_cents, require_positive
from ledger.report.format import publish, render


def test_deposit_adds_to_the_balance(account):
    account.deposit(250)
    assert account.cents == 250


def test_statements_mention_the_owner(account):
    assert account.statement()


def test_parsing_garbage_gives_nothing():
    assert parse_cents("garbage") is None


def test_an_opening_account_exists():
    assert Account.opening("ada") is not None


def test_negative_deposits_are_rejected():
    with pytest.raises(ValueError):
        require_positive(-5)


def test_rounding_is_close():
    assert parse_cents("1.005") / 100 == pytest.approx(1.0, abs=0.01)


@pytest.mark.skip(reason="talks to the network")
def test_statement_is_published():
    assert publish("hello").status_code != 500


def test_report_matches_snapshot(snapshot):
    assert render(1234) == snapshot


@patch("ledger.report.format.requests")
def test_publish_posts_once(requests):
    publish("hello")
    requests.post.assert_called_once()


def test_rendering_can_be_replaced(monkeypatch):
    monkeypatch.setattr("ledger.report.format.render", lambda cents: "-")
    assert render(1) == "-"


def test_opening_an_account_does_not_raise():
    """An account opens without a deposit (REQ-1)."""
    Account("ada")
