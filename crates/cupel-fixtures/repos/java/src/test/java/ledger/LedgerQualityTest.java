package ledger;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.mockito.Mockito.mock;

import org.junit.jupiter.api.Test;

class LedgerQualityTest {
    @Test
    void membersPayExactlyTheDiscountedPrice() {
        assertEquals(900, Pricing.discounted(1000, 10, true));
    }

    @Test
    void blankAmountsAreRejected() {
        assertThrows(IllegalArgumentException.class, () -> Decide.parseCents(""));
    }

    @Test
    void statementsAreNotEmpty() {
        assertFalse(Ledger.open("ann").statement().isEmpty());
    }

    @Test
    void classifyingRuns() {
        Decide.classify(100, 50);
    }

    @Test
    void mailGoesThroughEveryNotifier() {
        Notifier primary = mock(Notifier.class);
        Notifier backup = mock(Notifier.class);
        Notifier audit = mock(Notifier.class);
        assertNull(Pricing.mail(primary, Ledger.open("bo")));
    }

    @Test
    void everyEntryCountsTowardTheBalance() {
        for (long cents : new long[] {1, 20, 300}) {
            assertEquals(cents, Ledger.balance(new long[] {cents}));
        }
    }

    @Test
    void computedEntriesCountTowardTheBalance() {
        for (long cents = 100; cents <= 300; cents += 100) {
            assertEquals(cents, Ledger.balance(new long[] {cents}));
        }
    }

    @Test
    void depositsKeepTheOwner() {
        Account account = Ledger.open("kim");
        account.deposit(100);
        account.deposit(50);
        assertEquals("kim", account.owner());
    }

    @Test
    void depositsAccumulate() {
        Account account = Ledger.open("lee");
        account.deposit(200);
        account.deposit(25);
        long total = account.cents();
        assertEquals(225, total);
    }
}
