export class Circle {
  constructor(readonly radius: number) {}

  area(): number {
    return Math.PI * this.radius * this.radius;
  }

  static unit(): Circle {
    return new Circle(1);
  }

  private isValid(): boolean {
    return this.radius > 0;
  }
}

export const scale = (circle: Circle, factor: number): Circle =>
  new Circle(circle.radius * factor);

function perimeter(circle: Circle): number {
  function twice(value: number): number {
    return value * 2;
  }
  return twice(Math.PI * circle.radius);
}
