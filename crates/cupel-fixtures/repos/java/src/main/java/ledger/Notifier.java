package ledger;

/** Sends statements to account owners. */
public interface Notifier {
    /** Sends a statement; returns the message ID. */
    String send(String owner, String statement);
}
