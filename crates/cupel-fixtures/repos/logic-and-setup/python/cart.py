# ABOUTME: A shopping cart the setup tests arrange line by line before checking one total each.
# ABOUTME: Lines are (name, price, quantity); a discount is a share taken off the total.


class Cart:
    def __init__(self):
        self.lines = []
        self.discount = 0.0

    def add(self, name, price, quantity):
        self.lines.append((name, price, quantity))

    def total(self):
        gross = sum(price * quantity for _, price, quantity in self.lines)
        return round(gross * (1 - self.discount), 2)

    def count(self):
        return sum(quantity for _, _, quantity in self.lines)

    def names(self):
        return [name for name, _, _ in self.lines]
