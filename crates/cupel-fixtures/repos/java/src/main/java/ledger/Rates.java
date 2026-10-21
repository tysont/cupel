package ledger;

/** Charges computed from the reference cases' rates. */
public final class Rates {
    private Rates() {
    }

    public static long doubled(long amount) {
        return References.importedRate(amount);
    }
}
