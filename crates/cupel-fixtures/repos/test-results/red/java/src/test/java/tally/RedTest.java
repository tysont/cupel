package tally;

import static org.junit.jupiter.api.Assertions.assertEquals;

import org.junit.jupiter.api.Test;

class RedTest {
    @Test
    void reportsAWrongTotal() {
        assertEquals(5, Tally.total(new int[] {2, 2}), "totalling 2 and 2");
    }
}
