package ledger.depscycle.alpha;

import ledger.depscycle.beta.Beta;

/** One half of an import cycle. */
public final class Alpha {
    private Alpha() {
    }

    /** One more than beta. */
    public static int alpha() {
        return Beta.beta() + 1;
    }
}
