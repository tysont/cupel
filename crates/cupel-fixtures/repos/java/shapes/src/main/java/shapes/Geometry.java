package shapes;

/** Areas of plane figures. */
public final class Geometry {
    private Geometry() {
    }

    /** The area of a square with sides of `side`. */
    public static double square(double side) {
        return side * side;
    }

    /** A circle of a given radius. */
    public record Circle(double radius) {
        /** The circle's area. */
        public double area() {
            return Math.PI * radius * radius;
        }

        Circle scaled(double factor) {
            java.util.function.DoubleUnaryOperator times = value -> value * factor;
            return new Circle(times.applyAsDouble(radius));
        }
    }
}
