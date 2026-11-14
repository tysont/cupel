// ABOUTME: Three tests of the decoder: two expect its exception with a `try` ending in `fail`, one catches the
// ABOUTME: exception without expecting it, whose catch is test.logic.
package codec;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.fail;

import org.junit.jupiter.api.Assertions;
import org.junit.jupiter.api.Test;

class DecoderTest {
    @Test
    void decodeRejectsPunctuation() {
        try {
            Decoder.decode("-%8en$9m=>$m");
            fail("Expecting exception: IllegalArgumentException");
        } catch (IllegalArgumentException e) {
            assertEquals(Decoder.class.getName(), e.getStackTrace()[0].getClassName());
        }
    }

    @Test
    void decodeRejectsDigits() {
        try {
            Decoder.decode("abc1");
            Assertions.fail();
        } catch (IllegalArgumentException e) {
            assertEquals("Illegal character in abc1", e.getMessage());
        }
    }

    @Test
    void decodeCountsLettersOrExplains() {
        int length;
        try {
            length = Decoder.decode("abc");
        } catch (IllegalArgumentException e) {
            length = -1;
        }
        assertEquals(3, length);
    }
}
