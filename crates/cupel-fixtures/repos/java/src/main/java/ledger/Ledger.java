package ledger;

import java.util.HashMap;
import java.util.Map;
import java.util.UUID;
import java.util.function.Function;

import com.google.common.base.Strings;

import ledger.gen.Version;
import ledger.report.Format;
import shapes.Geometry;

/** The ledger: accounts by ID and the arithmetic over their entries. */
public final class Ledger {
    /** Bounds a single statement. */
    public static final int MAX_ENTRIES = 100;

    /** The ledger's display currency. */
    public static String currency = "USD";

    static Map<String, Account> registry = new HashMap<>();

    private Ledger() {
    }

    /** Opens an account for owner. */
    public static Account open(String owner) {
        Account account = new Account(UUID.randomUUID().toString(), owner);
        registry.put(account.id(), account);
        return account;
    }

    /** Sums entries. */
    public static long balance(long[] entries) {
        return scale(entries, 1);
    }

    static long scale(long[] entries, long factor) {
        long total = 0;
        for (long entry : entries) {
            total += entry * factor;
        }
        return total;
    }

    /** The area of a square room. */
    public static double floorArea(double side) {
        return Geometry.square(side);
    }

    /** The printed header of an account's statement. */
    public static String header(Account account) {
        Function<String, String> title = owner -> owner + " (" + Version.NUMBER + ")";
        return title.apply(account.owner()) + "\n" + Format.render(account.cents()) + Strings.repeat("=", 3);
    }
}
