import unittest

from ledger import balance


class LedgerTestCase(unittest.TestCase):
    def balances(self, entries, expected):
        self.assertEqual(balance(entries), expected)
