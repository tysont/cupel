def clamp_percent(value):
    """`value` limited to 0..100; tested by example only, so it has no property."""
    return max(0, min(100, value))
