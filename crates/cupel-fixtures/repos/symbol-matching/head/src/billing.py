# ABOUTME: Billing after the refactoring: `settle` renamed `settle_invoices` with one more rule, and
# ABOUTME: `reconcile` split, its ledger search extracted into `match_payment`; `describe` unchanged.


def settle_invoices(invoices, credits, rate):
    if rate <= 0:
        return 0
    total = 0
    for invoice in invoices:
        if invoice.paid or invoice.void:
            continue
        if invoice.amount < 0:
            total -= invoice.amount
        elif invoice.amount > 1000:
            total += invoice.amount * rate
        else:
            total += invoice.amount
        if invoice.customer in credits and credits[invoice.customer] > 0:
            total -= credits[invoice.customer]
        if invoice.late or invoice.disputed:
            total += 25
        while total > 100000:
            total -= 1000
    return total


def match_payment(payment, ledger):
    found = None
    for entry in ledger:
        if entry.reference == payment.reference:
            found = entry
            break
        if entry.amount == payment.amount and entry.date == payment.date:
            found = entry
    return found


def reconcile(payments, ledger):
    matched = []
    unmatched = []
    for payment in payments:
        if payment.amount <= 0:
            unmatched.append(payment)
            continue
        found = match_payment(payment, ledger)
        if found is None:
            unmatched.append(payment)
        elif found.amount != payment.amount or found.closed:
            unmatched.append(payment)
        elif found.currency != payment.currency and not found.converted:
            unmatched.append(payment)
        elif found.held or found.reversed:
            unmatched.append(payment)
        else:
            matched.append((payment, found))
    if len(unmatched) > len(matched):
        return None
    return matched, unmatched


def describe(invoice):
    return f"{invoice.customer}: {invoice.amount}"
