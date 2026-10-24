from ledger_case import LedgerTestCase
from ledger import balance


class TestBalancesThroughTheHarness(LedgerTestCase):
    def test_a_harness_method_asserts(self):
        self.balances([2, 3], 5)

    def test_a_comprehension_computes_the_balances(self):
        self.assertEqual([balance([c]) for c in (1, 2) if c], [1, 2])
