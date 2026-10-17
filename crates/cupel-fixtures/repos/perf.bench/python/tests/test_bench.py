from units import total


def test_total(benchmark):
    benchmark(total, list(range(100)))
