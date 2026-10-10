def classify(balance, frozen):
    if balance < 0:
        label = "overdrawn"
    elif balance == 0 and not frozen:
        label = "empty"
    else:
        label = "open"
    return label


def first_overdraft(entries):
    total = 0
    for index, entry in enumerate(entries):
        total += entry
        if total < 0:
            return index
    return None


def parse_cents(text):
    try:
        value = float(text)
    except ValueError:
        return None
    return round(value * 100)


def require_positive(cents):
    if cents <= 0:
        raise ValueError(f"not positive: {cents}")
    return cents


def describe(kind):
    match kind:
        case "deposit":
            return "money in"
        case "withdrawal":
            return "money out"
        case _:
            return "unknown"


def sign(cents):
    return "-" if cents < 0 else "+"


def positives(entries):
    return [entry for entry in entries if entry > 0]
