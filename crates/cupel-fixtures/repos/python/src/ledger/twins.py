def net_total(entries, fee):
    total = sum(entries)
    if total > fee:
        return total - fee
    return 0


def sum_after_charge(items, charge):
    amount = sum(items)
    if amount > charge:
        return amount - charge
    return 0


def net_total_floor(entries, fee):
    total = sum(entries)
    if total > fee:
        return total - fee
    return 10


def net_total_laid_out(entries, fee):
    """The total less the fee, or nothing when the fee is larger."""
    total   =   sum(
        entries,
    )  # everything

    if total > fee:  # the fee fits
        return total   -   fee
    return 0


def net_total_plus(entries, fee):
    total = sum(entries)
    if total > fee:
        return total + fee
    return 0


def net_total_capped(entries, fee):
    total = sum(entries)
    if total > fee:
        return total - fee
    elif total < 0:
        return 0
    return 0


def net_total_product(entries, fee):
    total = max(entries)
    if total > fee:
        return total - fee
    return 0
