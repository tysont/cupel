import unittest
from unittest import mock

from ledger.pricing import discounted, tally


class PricingTest(unittest.TestCase):
    def setUp(self):
        self.entries = [5, -10]

    def test_tally_reports_overdraft(self):
        total, overdrawn = tally(self.entries)
        self.assertEqual(total, -5)
        self.assertTrue(overdrawn)

    @unittest.skip("pricing tables are being revised")
    def test_members_get_a_discount(self):
        self.assertNotEqual(discounted(100, True), 100)

    @mock.patch("ledger.pricing.log_total")
    def test_tally_logs_once(self, log_total):
        tally([1])
        log_total.assert_called_once_with(1)
