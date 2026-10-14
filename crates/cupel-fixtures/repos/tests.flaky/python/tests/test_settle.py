"""A test that fails on every other run: state outside the process flips its outcome."""
import os
import unittest

MARKER = ".settled"


class SettleTest(unittest.TestCase):
    def test_settles_on_every_other_run(self):
        if os.path.exists(MARKER):
            os.remove(MARKER)
            self.fail("settled twice in a row")
        open(MARKER, "w").close()
