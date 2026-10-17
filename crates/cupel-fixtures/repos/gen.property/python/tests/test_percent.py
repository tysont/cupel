from units.percent import clamp_percent


def test_clamps_above_a_hundred():
    assert clamp_percent(250) == 100
