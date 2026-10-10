export class Circle {
  constructor(radius) {
    this.radius = radius;
  }

  area() {
    return Math.PI * this.radius * this.radius;
  }

  static unit() {
    return new Circle(1);
  }

  #isValid() {
    return this.radius > 0;
  }
}

export const scale = (circle, factor) => new Circle(circle.radius * factor);

function perimeter(circle) {
  function twice(value) {
    return value * 2;
  }
  return twice(Math.PI * circle.radius);
}
