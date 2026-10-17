package ledger;

/** Pairs of functions that differ in one way, for structural-hash conformance. */
final class Twins {
    private Twins() {
    }

    static long netTotal(long[] items, long fee) {
        long gross = sum(items);
        if (gross > fee) {
            return gross - fee;
        }
        return 0;
    }

    static long sumAfterCharge(long[] values, long charge) {
        long subtotal = sum(values);
        if (subtotal > charge) {
            return subtotal - charge;
        }
        return 0;
    }

    static long netTotalFloor(long[] items, long fee) {
        long gross = sum(items);
        if (gross > fee) {
            return gross - fee;
        }
        return 100;
    }

    static long netTotalLaidOut(long[] items, long fee) {
        // The same body as netTotal, laid out differently.
        long gross = sum(
            items
        );

        if (gross > fee) {
            return gross - fee; // what is left after the fee
        }
        return 0;
    }

    static long netTotalPlus(long[] items, long fee) {
        long gross = sum(items);
        if (gross > fee) {
            return gross + fee;
        }
        return 0;
    }

    static long netTotalCapped(long[] items, long fee) {
        long gross = sum(items);
        if (gross > fee) {
            return gross - fee;
        } else if (gross < 0) {
            return gross;
        }
        return 0;
    }

    static long netTotalProduct(long[] items, long fee) {
        long gross = product(items);
        if (gross > fee) {
            return gross - fee;
        }
        return 0;
    }

    static long sum(long[] items) {
        long total = 0;
        for (long item : items) {
            total += item;
        }
        return total;
    }

    static long product(long[] items) {
        long total = 1;
        for (long item : items) {
            total *= item;
        }
        return total;
    }
}
