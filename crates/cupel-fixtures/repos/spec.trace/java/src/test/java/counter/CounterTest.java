package counter;

import static org.junit.jupiter.api.Assertions.assertEquals;

import java.nio.file.Path;
import org.junit.jupiter.api.Test;

class CounterTest {
    /** The trace file `name` in the directory CUPEL_TRACE_DIR names, when it is set. */
    private static Path trace(String name) {
        String dir = System.getenv("CUPEL_TRACE_DIR");
        return dir == null ? null : Path.of(dir, name);
    }

    @Test
    void countsThenFinishes() {
        Counter counter = new Counter(trace("counts.jsonl"));
        counter.inc();
        counter.inc();
        counter.finish();
        assertEquals(2, counter.count());
    }

    @Test
    void bumps() {
        Counter counter = new Counter(trace("bumps.jsonl"));
        counter.inc();
        counter.bump();
        assertEquals(3, counter.count());
    }
}
