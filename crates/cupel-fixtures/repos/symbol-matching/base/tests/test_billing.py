# ABOUTME: The billing tests before the refactoring; the head commit renames the settle test with the
# ABOUTME: function it tests and keeps both of its assertions.
from billing import settle


class Invoice:
    def __init__(self, customer, amount, paid=False):
        self.customer = customer
        self.amount = amount
        self.paid = paid
        self.late = False
        self.disputed = False


def test_settle_skips_paid_invoices_and_applies_credits():
    invoices = [Invoice("acme", 100, paid=True), Invoice("acme", 200), Invoice("bolt", -50)]
    credits = {"acme": 10, "bolt": 0}
    total = settle(invoices, credits, 0.9)
    assert total == 240
    assert len(invoices) == 3
