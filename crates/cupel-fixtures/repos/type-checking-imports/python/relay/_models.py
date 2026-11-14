# ABOUTME: The models, which take their header type from the type aliases.
# ABOUTME: Its import of the aliases runs.
from ._types import Headers


class Request:
    headers: Headers = {}
