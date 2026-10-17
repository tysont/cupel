package ledger;

import net.jqwik.api.ForAll;
import net.jqwik.api.Property;

class LedgerPropertyTest {
    @Property
    boolean balanceOfOneEntryIsTheEntry(@ForAll long cents) {
        return Ledger.balance(new long[] {cents}) == cents;
    }
}
