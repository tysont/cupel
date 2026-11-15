# ABOUTME: A test asserting that the headers hold the token, which pins it, and one asserting only that they
# ABOUTME: leave a header out, which rules one value out and is weak (ADR-223).
from auth import headers


def test_the_token_is_in_the_headers():
    assert "Bearer abc" in str(headers("abc"))


def test_no_cookie_is_sent():
    assert "Cookie" not in headers("abc")
