package tally;

import static org.junit.jupiter.api.Assertions.assertEquals;

import org.junit.jupiter.api.Disabled;
import org.junit.jupiter.api.Test;

class TallyTest {
    @Test
    void totalsAList() {
        assertEquals(6, Tally.total(new int[] {1, 2, 3}));
    }

    @Test
    @Disabled("kept to show a skipped test")
    void totalsAHugeList() {
    }
}
