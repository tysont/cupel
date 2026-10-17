package ledger.report;

/** Formats amounts for printing. */
public final class Format {
    private Format() {
    }

    /** Cents as a currency amount. */
    public static String render(long cents) {
        return String.format("%d.%02d", cents / 100, cents % 100);
    }
}
