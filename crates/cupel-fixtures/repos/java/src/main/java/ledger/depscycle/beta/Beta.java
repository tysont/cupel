package ledger.depscycle.beta;

import ledger.depscycle.alpha.Alpha;

/** The other half of an import cycle. */
public final class Beta {
    private Beta() {
    }

    /** Two. */
    public static int beta() {
        return 2;
    }

    /** Both halves. */
    public static int[] both() {
        return new int[] {Alpha.alpha(), beta()};
    }
}
