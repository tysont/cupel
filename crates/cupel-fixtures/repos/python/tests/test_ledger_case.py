from ledger_case import LedgerTestCase


class TestBalancesThroughTheHarness(LedgerTestCase):
    def test_a_harness_method_asserts(self):
        self.balances([2, 3], 5)
