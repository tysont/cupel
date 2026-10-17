package ledger;

/** One balance in a static field, which the tests share. */
public final class Ledger {
    /** The running total of every deposit. */
    public static int balance;

    private Ledger() {
    }

    /** Adds amount to the balance. */
    public static void deposit(int amount) {
        balance += amount;
    }
}
