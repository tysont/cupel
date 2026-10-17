package tally;

/** A passing test suite for Cupel's runner producers. */
public final class Tally {
    private Tally() {
    }

    /** The sum of values. */
    public static int total(int[] values) {
        int sum = 0;
        for (int value : values) {
            sum += value;
        }
        return sum;
    }
}
