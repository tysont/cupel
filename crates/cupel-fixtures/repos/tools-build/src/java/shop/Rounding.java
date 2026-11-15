// ABOUTME: A Java helper of the tools-build fixture, compiled by the Clojure project's own build (its :deps/prep-lib)
// ABOUTME: rather than by Maven or Gradle.
package shop;

public final class Rounding {
    private Rounding() {}

    public static long toCents(double amount) {
        return Math.round(amount * 100);
    }
}
