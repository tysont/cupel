# ABOUTME: Written by Cupel's property engine (ADR-095) and loaded with `-p cupel_property`: after the project's
# ABOUTME: conftests, every Hypothesis test runs with Cupel's case budget, unpinned, and without an example database.
import pytest


@pytest.hookimpl(trylast=True)
def pytest_configure(config):
    from hypothesis import settings

    settings.register_profile(
        "cupel",
        parent=settings.default,
        max_examples={cases},
        derandomize=False,
        database=None,
    )
    settings.load_profile("cupel")
