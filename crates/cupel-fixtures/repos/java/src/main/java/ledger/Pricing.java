package ledger;

import java.util.logging.Logger;

/** Prices and running totals. */
final class Pricing {
    private static final Logger LOG = Logger.getLogger("ledger");

    /** A running total and whether it went below the overdraft line. */
    record Tally(long total, boolean overdrawn) {
    }

    private Pricing() {
    }

    static long discounted(long price, long percent, boolean member) {
        long cut = price * percent / 100;
        if (member && cut != 0) {
            return price - cut;
        }
        return price;
    }

    static Tally tally(long[] entries) {
        long total = 0;
        boolean overdrawn = false;
        for (long entry : entries) {
            total += entry;
            if (total < -1) {
                overdrawn = true;
            }
        }
        logTotal(total);
        return new Tally(total, overdrawn);
    }

    static void logTotal(long total) {
        LOG.info("total " + total);
    }

    /** Sends an account's statement through notifier. */
    public static String mail(Notifier notifier, Account account) {
        return notifier.send(account.owner(), account.statement());
    }
}
