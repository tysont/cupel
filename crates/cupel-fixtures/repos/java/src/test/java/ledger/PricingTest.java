package ledger;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;
import static org.mockito.ArgumentMatchers.anyString;
import static org.mockito.ArgumentMatchers.eq;
import static org.mockito.Mockito.mock;
import static org.mockito.Mockito.verify;
import static org.mockito.Mockito.when;

import au.com.origin.snapshots.Expect;
import au.com.origin.snapshots.junit5.SnapshotExtension;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.extension.ExtendWith;

@ExtendWith(SnapshotExtension.class)
class PricingTest {
    private Expect expect;

    @Test
    void tallyReportsOverdraft() {
        Pricing.Tally tally = Pricing.tally(new long[] {5, -10, 2});
        assertEquals(-3, tally.total());
        assertTrue(tally.overdrawn());
    }

    @Test
    void parseCentsAcceptsDigits() {
        assertEquals(125, Decide.parseCents(" 125 "));
    }

    @Test
    void parseCentsRejectsBlanks() {
        assertThrows(IllegalArgumentException.class, () -> Decide.parseCents("  "));
    }

    @Test
    void discountSnapshot() {
        expect.toMatchSnapshot(Pricing.discounted(200, 50, true));
    }

    @Test
    void mailNotifiesTheOwner() {
        Notifier notifier = mock(Notifier.class);
        when(notifier.send(eq("ada"), anyString())).thenReturn("sent");
        assertEquals("sent", Pricing.mail(notifier, Ledger.open("ada")));
        verify(notifier).send(eq("ada"), anyString());
    }
}
