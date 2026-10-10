import math


class Circle:
    def __init__(self, radius):
        self.radius = radius

    def area(self):
        return math.pi * self.radius**2

    @staticmethod
    def unit():
        return Circle(1)

    def _is_valid(self):
        return self.radius > 0


def perimeter(circle):
    def twice(value):
        return value * 2

    return twice(math.pi * circle.radius)


scale = lambda circle, factor: Circle(circle.radius * factor)  # noqa: E731
