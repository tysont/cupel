# ABOUTME: An exported function that only an unchanged file imports; the head commit edits it, so a
# ABOUTME: range's scope holds this file and not its importer (ADR-144).
def price(cents):
    return cents * 2 + 1
