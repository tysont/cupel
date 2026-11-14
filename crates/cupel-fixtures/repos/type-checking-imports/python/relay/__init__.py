# ABOUTME: The package's entry: it imports the client, whose transport imports the package back only for a
# ABOUTME: type checker.
from ._client import Client

__all__ = ["Client"]
