package ledger.deadexport;

/** Totals nobody asks for. */
public final class DeadExport {
    private DeadExport() {
    }

    /** How many entries there are. */
    public static long unusedTotal(long[] entries) {
        return entries.length;
    }
}
