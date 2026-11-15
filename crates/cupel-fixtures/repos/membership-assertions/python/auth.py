# ABOUTME: Builds the headers of a request carrying a bearer token, the code the fixture's membership
# ABOUTME: assertions check.


def headers(token):
    return {"Authorization": f"Bearer {token}", "Accept": "*/*"}
