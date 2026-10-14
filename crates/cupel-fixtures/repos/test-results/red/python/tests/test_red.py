"""The red-baseline overlay: one failing test."""
import unittest

from tally import total


class RedTest(unittest.TestCase):
    def test_reports_a_wrong_total(self):
        print("totalling 2 and 2")
        self.assertEqual(total([2, 2]), 5)
