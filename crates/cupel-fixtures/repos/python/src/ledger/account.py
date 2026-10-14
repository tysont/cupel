from typing import Protocol

from ledger.report.format import render


class Audited(Protocol):
    def audit_trail(self) -> list[str]: ...

    def is_clean(self) -> bool:
        return not self.audit_trail()


class Account:
    """A named balance in cents."""

    def __init__(self, owner: str):
        self.owner = owner
        self.cents = 0

    @staticmethod
    def opening(owner: str):
        return Account(owner)

    def deposit(self, cents: int) -> None:
        self.cents += cents

    def statement(self) -> str:
        def line(label, value):
            return f"{label}: {value}"

        header = lambda: line("owner", self.owner)  # noqa: E731
        return header() + "\n" + render(self.cents)

    def _audit(self):
        return self.__check()

    def __check(self):
        return self.cents >= 0
