from hypothesis import settings

# The everyday suite runs each property briefly and deterministically; Cupel's property run loads
# its own profile over this one.
settings.register_profile("everyday", max_examples=10, derandomize=True, database=None)
settings.load_profile("everyday")
