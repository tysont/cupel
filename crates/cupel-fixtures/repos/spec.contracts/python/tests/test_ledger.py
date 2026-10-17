from ledger import Account, credit, half, share


def test_shares_nothing_among_no_parts():
    assert share(10, 0) == 0


def test_halves_one():
    assert half(1) == 0


def test_credits_within_the_limit():
    assert credit(5, 10) == 15


def test_posts_three_entries():
    account = Account()
    for amount in [1, 2, 3]:
        account.post(amount)
    assert len(account.entries) == 3
