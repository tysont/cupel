import os

import pytest

from counter import Counter


@pytest.fixture
def trace(request):
    """Opens a trace file in the directory CUPEL_TRACE_DIR names, when it is set."""
    files = []

    def open_trace(name):
        directory = os.environ.get("CUPEL_TRACE_DIR")
        if directory is None:
            return None
        files.append(open(os.path.join(directory, name), "w"))
        return files[-1]

    yield open_trace
    for file in files:
        file.close()


def test_counts_then_finishes(trace):
    counter = Counter(trace("counts.jsonl"))
    counter.inc()
    counter.inc()
    counter.finish()
    assert counter.count == 2


def test_bumps(trace):
    counter = Counter(trace("bumps.jsonl"))
    counter.inc()
    counter.bump()
    assert counter.count == 3
