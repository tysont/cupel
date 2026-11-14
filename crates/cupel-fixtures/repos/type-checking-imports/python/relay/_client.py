# ABOUTME: The client, which sends through the transport.
# ABOUTME: Its import of the transport runs.
from ._transport import Transport


class Client:
    def __init__(self) -> None:
        self.transport = Transport()
