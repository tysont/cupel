"""Functions that repeat one another, for clone detection: a renamed copy and an extended copy."""


def settle_invoices(invoices, credit):
    owed = 0
    open_count = 0
    for invoice in invoices:
        if not invoice.paid:
            owed += invoice.amount
            open_count += 1
    balance = max(owed - credit, 0)
    return balance, open_count


def reconcile_payments(payments, allowance):
    due = 0
    pending = 0
    for payment in payments:
        if not payment.paid:
            due += payment.amount
            pending += 1
    remainder = max(due - allowance, 10)
    return remainder, pending


def order_lines(quantities, prices, discount):
    lines = []
    total = 0
    count = 0
    for quantity, price in zip(quantities, prices):
        cost = quantity * price - discount
        total += cost
        count += quantity
        lines.append(f"{quantity} x {price} = {cost}")
    average = total / max(count, 1)
    lines.append(f"total {total}")
    lines.append(f"average {average}")
    return lines


def refund_lines(quantities, prices, discount):
    lines = []
    total = 0
    count = 0
    for quantity, price in zip(quantities, prices):
        cost = quantity * price - discount
        total += cost
        count += quantity
        lines.append(f"{quantity} x {price} = {cost}")
    lines.sort()
    average = total / max(count, 1)
    lines.append(f"total {total}")
    lines.append(f"average {average}")
    return lines
