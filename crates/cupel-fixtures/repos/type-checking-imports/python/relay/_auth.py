# ABOUTME: Authentication, which builds a request from the models.
# ABOUTME: Its import of the models runs.
from ._models import Request


class Auth:
    def sign(self) -> Request:
        return Request()
