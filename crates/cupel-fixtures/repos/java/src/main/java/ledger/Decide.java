package ledger;

/** Decisions over amounts. */
final class Decide {
    private Decide() {
    }

    static String classify(long amount, long limit) {
        String kind = "credit";
        if (amount < 0) {
            kind = "debit";
        } else if (amount > limit && limit > 0) {
            kind = "large";
        }
        return kind;
    }

    static int firstOverdraft(long[] balances) {
        for (int index = 0; index < balances.length; index++) {
            if (balances[index] < 0) {
                return index;
            }
        }
        return -1;
    }

    static long parseCents(String text) {
        String trimmed = text.strip();
        if (trimmed.isEmpty()) {
            throw new IllegalArgumentException("empty amount");
        }
        try {
            return Long.parseLong(trimmed);
        } catch (NumberFormatException error) {
            return -1;
        }
    }

    static String describe(int code) {
        switch (code) {
            case 0:
                return "ok";
            case 1:
            case 2:
                return "retry";
            default:
                return "fail";
        }
    }

    static String sign(long amount) {
        return amount < 0 ? "-" : "+";
    }

    static String size(int count) {
        return switch (count) {
            case 0 -> "none";
            case 1 -> "one";
            default -> "many";
        };
    }

    static long drain(java.util.Iterator<Long> entries) {
        long total = 0;
        while (entries.hasNext()) {
            total += entries.next();
        }
        do {
            total -= 1;
        } while (total > 1000);
        return total;
    }
}
