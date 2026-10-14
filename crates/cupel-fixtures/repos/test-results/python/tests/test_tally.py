"""Tests of tally, written for unittest so pytest and unittest both run them."""
import unittest

from tally import total


class TotalTest(unittest.TestCase):
    def test_totals_a_list(self):
        self.assertEqual(total([1, 2, 3]), 6)

    @unittest.skip("kept to show a skipped test")
    def test_totals_a_huge_list(self):
        self.assertEqual(total([1] * 1000), 1000)
