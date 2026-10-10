import pytest

from ledger import Account


@pytest.fixture
def account():
    return Account("ada")
