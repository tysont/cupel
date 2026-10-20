# ABOUTME: Written by Cupel's property engine (ADR-095) and loaded with `-p cupel_property`: after the project's
# ABOUTME: conftests, every Hypothesis test runs with Cupel's case budget, unpinned, and without an example database.
from __future__ import annotations

import pytest

# The run's case budget: Cupel writes it into this string when it writes the plugin.
CASES = int("{cases}")


@pytest.hookimpl(trylast=True)
def pytest_configure(config: pytest.Config) -> None:
    from hypothesis import settings

    settings.register_profile(
        "cupel",
        parent=settings.default,
        max_examples=CASES,
        derandomize=False,
        database=None,
    )
    settings.load_profile("cupel")
