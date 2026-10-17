package ledger;

import static org.junit.jupiter.api.Assertions.assertEquals;

import org.junit.jupiter.api.Test;

class BalanceTest {
    /** Passes only while no test before it has deposited: order-dependent. */
    @Test
    void balanceStartsAtZero() {
        assertEquals(0, Ledger.balance);
    }
}
