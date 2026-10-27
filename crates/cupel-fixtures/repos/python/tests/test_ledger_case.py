from ledger_case import LedgerTestCase
from ledger import balance


class TestBalancesThroughTheHarness(LedgerTestCase):
    def test_a_harness_method_asserts(self):
        self.balances([2, 3], 5)

    def test_a_comprehension_computes_the_balances(self):
        self.assertEqual([balance([c]) for c in (1, 2) if c], [1, 2])

    def test_balances_of_pairs(self):
        self.balances([1, 1], 2)
        self.balances([2, 2], 4)
        self.balances([3, 3], 6)
        self.balances([4, 4], 8)

    def test_balances_of_larger_pairs(self):
        self.balances([5, 5], 10)
        self.balances([6, 6], 12)
        self.balances([7, 7], 14)
        self.balances([8], 8)

    def test_a_lambda_chooses_each_entry(self):
        self.assertEqual(balance(list(map(lambda c: c if c > 0 else 0, [2, -1]))), 2)
