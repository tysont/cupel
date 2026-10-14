def discounted(price, member):
    cut = price * 15 / 100
    if member and cut != 0:
        return price - cut
    return price


def tally(entries):
    total = 0
    overdrawn = False
    for entry in entries:
        total += entry
        if total < -1:
            overdrawn = True
    log_total(total)
    return total, overdrawn


def has_price(item):
    return item.price is not None


def log_total(total):
    print(total)
