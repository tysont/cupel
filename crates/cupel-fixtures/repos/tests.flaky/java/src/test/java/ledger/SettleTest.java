package ledger;

import static org.junit.jupiter.api.Assertions.fail;

import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;

import org.junit.jupiter.api.Test;

class SettleTest {
    /** Fails on every other run: state outside the process flips its outcome between identical runs. */
    @Test
    void settlesOnEveryOtherRun() throws IOException {
        Path marker = Path.of(".settled");
        if (Files.exists(marker)) {
            Files.delete(marker);
            fail("settled twice in a row");
        }
        Files.createFile(marker);
    }
}
