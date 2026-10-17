package ledger.deadhelpers;

final class DeadHelpers {
    private DeadHelpers() {
    }

    static long forgottenRate(long amount) {
        return amount / 2;
    }
}
