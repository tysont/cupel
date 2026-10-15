"""A balance kept in a module global, which the tests share; written for unittest so pytest and
unittest both run them."""
import unittest

BALANCE = []


class LedgerTest(unittest.TestCase):
    def test_balance_starts_at_zero(self):
        # Passes only while no test before it has deposited: order-dependent.
        self.assertEqual(sum(BALANCE), 0)

    def test_deposit_changes_balance(self):
        # Leaves the shared balance changed for every test after it.
        BALANCE.append(5)
        self.assertGreaterEqual(sum(BALANCE), 5)
