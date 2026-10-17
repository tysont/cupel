package ledger;

import java.util.List;

/** Implemented by anything with an audit trail. */
public interface Audited {
    /** Every change, oldest first. */
    List<String> auditTrail();
}
