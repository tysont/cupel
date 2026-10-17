"""Shares, halves, and accounts, with contracts the project's own test runs leave unchecked."""

import icontract


@icontract.require(lambda parts: parts > 0, enabled=icontract.SLOW)
def share(total, parts):
    return total // parts if parts else 0


@icontract.ensure(lambda result: result > 0, enabled=icontract.SLOW)
def half(amount):
    return amount // 2


@icontract.require(lambda amount: amount <= 1000)
def credit(balance, amount):
    return balance + amount


@icontract.invariant(lambda self: len(self.entries) <= 2, enabled=icontract.SLOW)
class Account:
    def __init__(self):
        self.entries = []

    def post(self, amount):
        self.entries.append(amount)
