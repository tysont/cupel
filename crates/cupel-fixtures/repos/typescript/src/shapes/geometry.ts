export abstract class Shape {
  abstract area(): number;

  describe(): string {
    return `area ${this.area()}`;
  }
}

export class Circle extends Shape {
  constructor(readonly radius: number) {
    super();
  }

  area(): number {
    return Math.PI * this.radius * this.radius;
  }

  static unit(): Circle {
    return new Circle(1);
  }

  private isValid(): boolean {
    return this.radius > 0;
  }

  #cache(): void {}
}

export const scale = (circle: Circle, factor: number): Circle =>
  new Circle(circle.radius * factor);

function perimeter(circle: Circle): number {
  function twice(value: number): number {
    return value * 2;
  }
  return twice(Math.PI * circle.radius);
}
