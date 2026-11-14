# ABOUTME: Prices: a discount the tests run and a surcharge none runs.
# ABOUTME: The surcharge is the one uncovered function in the included files.


def discount(price, percent):
    return price - price * percent / 100


def surcharge(price):
    total = price * 1.2
    return round(total, 2)
