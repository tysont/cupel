# ABOUTME: Type aliases, which name authentication only for a type checker, under `if TYPE_CHECKING:`, as
# ABOUTME: httpx/_types.py does.
from typing import TYPE_CHECKING, Dict

if TYPE_CHECKING:
    from ._auth import Auth

Headers = Dict[str, str]
AuthTypes = "Auth"
