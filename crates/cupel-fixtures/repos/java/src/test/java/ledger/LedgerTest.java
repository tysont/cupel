package ledger;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNotEquals;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertTrue;

import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Disabled;
import org.junit.jupiter.api.Test;

class LedgerTest {
    private Account account;

    @BeforeEach
    void openAccount() {
        account = Ledger.open("ada");
    }

    @AfterEach
    void closeAccount() {
        Ledger.registry.remove(account.id());
    }

    @Test
    void depositAddsToTheBalance() {
        account.deposit(250);
        assertEquals(250, account.cents());
    }

    @Test
    void membersGetADiscount() {
        assertTrue(Pricing.discounted(200, 50, true) < 200);
    }

    @Test
    void newAccountsAreRegistered() {
        assertNotNull(Ledger.registry.get(account.id()));
    }

    @Test
    void parsingGarbageFails() {
        assertTrue(Decide.parseCents("12x") < 0);
    }

    @Test
    void newAccountsHaveDistinctIds() {
        assertNotEquals(account.id(), Ledger.open("grace").id());
    }

    @Test
    void classify() {
        String[][] cases = {{"-5", "debit"}, {"500", "large"}};
        for (String[] each : cases) {
            assertEquals(each[1], Decide.classify(Long.parseLong(each[0]), 100));
        }
    }

    @Test
    @Disabled("talks to the network")
    void statementIsMailed() {
        assertTrue(!account.statement().isEmpty());
    }

    @Test
    void parseCentsTrimsSpaces() {
        assertEquals(42, Decide.parseCents(" 42 "));
    }

    @Test
    void openingAnAccountDoesNotThrow() {
        Ledger.open("ada");
    }
}
