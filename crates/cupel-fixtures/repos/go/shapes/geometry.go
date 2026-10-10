package shapes

import "math"

type Circle struct {
	Radius float64
}

func NewCircle(radius float64) *Circle {
	return &Circle{Radius: radius}
}

func (c *Circle) Area() float64 {
	return math.Pi * c.Radius * c.Radius
}

func (c Circle) scaled(factor float64) Circle {
	double := func(value float64) float64 {
		return value * factor
	}
	return Circle{Radius: double(c.Radius)}
}

func Square(side float64) float64 {
	return side * side
}
