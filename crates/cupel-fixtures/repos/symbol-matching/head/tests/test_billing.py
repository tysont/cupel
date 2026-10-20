# ABOUTME: The billing tests after the refactoring: the settle test renamed with the function it tests,
# ABOUTME: its invoices given a `void` flag, both of its assertions kept.
from billing import settle_invoices


class Invoice:
    def __init__(self, customer, amount, paid=False):
        self.customer = customer
        self.amount = amount
        self.paid = paid
        self.void = False
        self.late = False
        self.disputed = False


def test_settled_totals_skip_paid_invoices_and_apply_credits():
    invoices = [Invoice("acme", 100, paid=True), Invoice("acme", 200), Invoice("bolt", -50)]
    credits = {"acme": 10, "bolt": 0}
    total = settle_invoices(invoices, credits, 0.9)
    assert total == 240
    assert len(invoices) == 3
