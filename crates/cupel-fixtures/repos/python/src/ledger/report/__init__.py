from units import cents

from . import format


def summary(amount):
    return format.render(cents(amount))
