// ABOUTME: JMH benchmarks in a module of their own, as jackson-core's jmh/StringWriteBench: one method per
// ABOUTME: input, repeating the same harness, which makes them clones of each other by design.
package codec.jmh;

import org.openjdk.jmh.annotations.Benchmark;
import org.openjdk.jmh.infra.Blackhole;

import codec.Escaper;

public class EscapeBench {
    @Benchmark
    public void escapeAscii(Blackhole sink) {
        StringBuilder text = new StringBuilder();
        for (int i = 0; i < 100; i++) {
            text.append("ascii").append(i);
        }
        String escaped = Escaper.escape(text.toString());
        sink.consume(escaped.length());
        sink.consume(escaped);
    }

    @Benchmark
    public void escapeQuoted(Blackhole sink) {
        StringBuilder text = new StringBuilder();
        for (int i = 0; i < 100; i++) {
            text.append("quote\"").append(i);
        }
        String escaped = Escaper.escape(text.toString());
        sink.consume(escaped.length());
        sink.consume(escaped);
    }
}
