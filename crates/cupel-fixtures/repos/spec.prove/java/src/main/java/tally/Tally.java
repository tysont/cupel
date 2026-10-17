package tally;

/** The tally whose arithmetic proofs/tally.dfy proves. */
public final class Tally {
    private Tally() {}

    /** The sum of values, as proofs/tally.dfy proves `Total` computes it. */
    public static long total(long[] values) {
        long total = 0;
        for (long value : values) {
            total += value;
        }
        return total;
    }
}
