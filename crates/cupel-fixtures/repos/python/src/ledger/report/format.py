import requests


def render(cents):
    return f"{cents / 100:.2f}"


def publish(text):
    return requests.post("https://ledger.invalid/statements", data=text)
