package ledger;

import java.util.List;

/** Holds a running balance in cents. */
public class Account implements Audited {
    private final String id;
    private final String owner;
    private long cents;

    /** An empty account. */
    public Account(String id, String owner) {
        this.id = id;
        this.owner = owner;
    }

    /** The account's ID. */
    public String id() {
        return id;
    }

    /** Who holds the account. */
    public String owner() {
        return owner;
    }

    long cents() {
        return cents;
    }

    /** Adds cents to the balance. */
    public void deposit(long amount) {
        cents += amount;
    }

    /** Renders the account for printing. */
    public String statement() {
        return Ledger.header(this);
    }

    @Override
    public List<String> auditTrail() {
        return List.of(id);
    }

    private boolean audited() {
        return cents >= 0;
    }
}
