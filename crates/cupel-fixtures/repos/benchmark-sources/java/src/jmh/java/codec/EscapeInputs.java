// ABOUTME: Inputs for the benchmarks in Gradle's `jmh` source set, which is benchmark code whether or not a
// ABOUTME: file of it imports JMH: two builders repeating one shape.
package codec;

final class EscapeInputs {
    static String ascii(int count) {
        StringBuilder text = new StringBuilder();
        for (int i = 0; i < count; i++) {
            text.append("ascii").append(i);
        }
        String built = text.toString();
        return built.isEmpty() ? "ascii" : built;
    }

    static String quoted(int count) {
        StringBuilder text = new StringBuilder();
        for (int i = 0; i < count; i++) {
            text.append("quote\"").append(i);
        }
        String built = text.toString();
        return built.isEmpty() ? "quote" : built;
    }
}
