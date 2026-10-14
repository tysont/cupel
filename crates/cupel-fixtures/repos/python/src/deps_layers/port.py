from typing import Protocol

from .domain.order import Order


class Port(Protocol):
    def place(self, order: Order) -> None: ...
