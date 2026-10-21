package ledger;

import java.lang.reflect.Method;

/**
 * Reference cases for static.dead-code (ADR-135): an exported rate, one only another class
 * calls, one only reflection by a computed name reaches, and one nothing names.
 */
public final class References {
    private References() {
    }

    public static long exportedRate(long amount) {
        return amount * 3;
    }

    static long importedRate(long amount) {
        return amount + amount;
    }

    private static long tieredRate(long amount) {
        if (amount > 100) {
            return amount / 3;
        }
        return amount;
    }

    private static long orphanRate(long amount) {
        return amount - amount / 4;
    }

    public static long rateNamed(String kind, long amount) throws ReflectiveOperationException {
        Method rate = References.class.getDeclaredMethod(kind + "Rate", long.class);
        return (long) rate.invoke(null, amount);
    }
}
