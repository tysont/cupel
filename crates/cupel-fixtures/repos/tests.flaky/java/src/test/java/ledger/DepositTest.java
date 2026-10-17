package ledger;

import static org.junit.jupiter.api.Assertions.assertTrue;

import org.junit.jupiter.api.Test;

class DepositTest {
    /** Leaves the shared balance changed for every test after it. */
    @Test
    void depositChangesBalance() {
        Ledger.deposit(5);
        assertTrue(Ledger.balance >= 5);
    }
}
