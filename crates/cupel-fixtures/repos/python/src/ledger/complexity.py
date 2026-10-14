"""Ledger helpers of varying shape: each of the first four trips one complexity limit."""


def label(amount, limit):
    if amount < 0:
        name = "refund"
    elif amount == 0:
        name = "empty"
    elif amount > limit * 100:
        name = "huge"
    elif amount > limit * 10:
        name = "large"
    elif amount > limit:
        name = "over"
    elif amount == limit:
        name = "at-limit"
    elif amount > limit / 2:
        name = "high"
    elif amount > limit / 4:
        name = "medium"
    elif amount > limit / 10:
        name = "low"
    elif amount > 1:
        name = "tiny"
    else:
        name = "unit"
    return name


def settle(batches, limit):
    settled = 0
    for batch in batches:
        if batch:
            for amount in batch:
                if amount > 0:
                    while settled < limit:
                        settled += amount
    return settled


def open_account(owner, currency, limit, overdraft, branch, opened):
    return f"{owner}/{currency}/{limit}/{overdraft}/{branch}/{opened}"


def statement_template():
    return [
        "ACME LEDGER",
        "===========",
        "",
        "Account holder:",
        "Account number:",
        "Branch:",
        "Currency:",
        "Statement period:",
        "",
        "Opening balance",
        "---------------",
        "Balance brought forward:",
        "Pending credits:",
        "Pending debits:",
        "",
        "Credits",
        "-------",
        "Deposits:",
        "Transfers in:",
        "Interest:",
        "Refunds:",
        "Adjustments:",
        "",
        "Debits",
        "------",
        "Withdrawals:",
        "Transfers out:",
        "Card payments:",
        "Direct debits:",
        "Standing orders:",
        "Fees:",
        "Charges:",
        "",
        "Closing balance",
        "---------------",
        "Balance carried forward:",
        "Available balance:",
        "Overdraft limit:",
        "Overdraft used:",
        "",
        "Notices",
        "-------",
        "Rates changed this period.",
        "Fees are listed above.",
        "Keep this statement for your records.",
        "Report errors within thirty days.",
        "",
        "Contact",
        "-------",
        "Telephone:",
        "Email:",
        "Post:",
        "",
        "Opening hours",
        "-------------",
        "Weekdays: 9 to 5",
        "Saturday: 9 to 12",
        "Sunday: closed",
        "",
        "Page 1 of 1",
        "End of statement",
    ]


def net(credits, debits):
    return credits - debits


class Statement:
    def __init__(self, owner):
        self.owner = owner

    def line(self, amount):
        return f"{self.owner}: {amount}"


def month(number):
    """Names a month by its number: one match, however many cases it has."""
    match number:
        case 1:
            return "jan"
        case 2:
            return "feb"
        case 3:
            return "mar"
        case 4:
            return "apr"
        case 5:
            return "may"
        case 6:
            return "jun"
        case 7:
            return "jul"
        case 8:
            return "aug"
        case 9:
            return "sep"
        case 10:
            return "oct"
        case 11:
            return "nov"
        case 12:
            return "dec"
        case _:
            return "none"
