package counter;

import java.io.IOException;
import java.io.UncheckedIOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.StandardOpenOption;

/** Counts up and logs each step it takes to a trace file, when it has one. */
public final class Counter {
    private int count;
    private boolean done;
    private final Path trace;

    public Counter(Path trace) {
        this.trace = trace;
        log(null);
    }

    public void inc() {
        count += 1;
        log("Inc");
    }

    public void bump() {
        count += 2;
        log("Inc");
    }

    public void finish() {
        done = true;
        log("Finish");
    }

    public int count() {
        return count;
    }

    private void log(String action) {
        if (trace == null) {
            return;
        }
        String state = "{\"count\": " + count + ", \"done\": " + done + "}";
        String step = action == null
            ? "{\"state\": " + state + "}\n"
            : "{\"action\": \"" + action + "\", \"state\": " + state + "}\n";
        try {
            Files.writeString(trace, step, StandardOpenOption.CREATE, StandardOpenOption.APPEND);
        } catch (IOException e) {
            throw new UncheckedIOException(e);
        }
    }
}
