# ABOUTME: The transport, which names the package's client in an annotation only, so it imports the package
# ABOUTME: under `if typing.TYPE_CHECKING:`, as httpx/_transports/default.py does.
import typing

if typing.TYPE_CHECKING:
    import relay


class Transport:
    def bind(self, client: "relay.Client") -> None:
        self.client = client
