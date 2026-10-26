package ledger;

import static org.junit.jupiter.api.Assertions.assertEquals;

import org.junit.jupiter.api.Test;

class LedgerHelperTest {
    private static void checkBalance(final long expected, final long... entries) {
        assertEquals(expected, Ledger.balance(entries));
    }

    @Test
    void aHelperOfTheClassAsserts() {
        checkBalance(30, 10, 20);
    }
}
