package grade;

import static org.junit.jupiter.api.Assertions.assertEquals;

import org.junit.jupiter.api.Test;

class GradeTest {
    @Test
    void totalsAList() {
        assertEquals(6, Grade.total(new int[] {1, 2, 3}));
    }

    @Test
    void classifiesPassingScores() {
        assertEquals("A", Grade.classify(95));
        assertEquals("pass", Grade.classify(60));
    }
}
