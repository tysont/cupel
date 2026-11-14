// ABOUTME: A Java helper of the tools-build fixture, compiled by the Clojure project's tools.build rather than by
// ABOUTME: Maven or Gradle.
package shop;

public final class Rounding {
    private Rounding() {}

    public static long toCents(double amount) {
        return Math.round(amount * 100);
    }
}
