package ledger.depslayers.domain;

import ledger.depslayers.infra.Store;

/** An order and where it is kept. */
public record Order(Store store) {
    /** The rows the order's store holds. */
    public int rows() {
        return store.rows();
    }
}
