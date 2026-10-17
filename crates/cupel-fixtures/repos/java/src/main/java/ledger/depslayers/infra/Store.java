package ledger.depslayers.infra;

/** Where orders are kept. */
public record Store(int rows) {
    /** An empty store. */
    public static Store empty() {
        return new Store(0);
    }
}
