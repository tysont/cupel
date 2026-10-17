package ledger.depslayers.port;

import ledger.depslayers.domain.Order;

/** Where orders are placed. */
public interface Port {
    /** Places an order. */
    void place(Order order);
}
