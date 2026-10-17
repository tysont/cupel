package quarter;

/** Quarters of an amount. */
public final class Quarter {
    private Quarter() {
    }

    /** A quarter of cents, rounded down. */
    public static long quarter(long cents) {
        return Math.floorDiv(cents, 4);
    }
}
