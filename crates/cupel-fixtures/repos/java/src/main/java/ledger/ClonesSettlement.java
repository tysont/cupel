package ledger;

import java.util.ArrayList;
import java.util.List;

/** Functions that repeat one another, for clone detection: renamed, extended, and exact copies. */
final class ClonesSettlement {
    /** An amount owed and whether it is paid. */
    record Invoice(long amount, boolean paid) {
    }

    private ClonesSettlement() {
    }

    static long[] settleInvoices(List<Invoice> invoices, long credit) {
        long owed = 0;
        long open = 0;
        for (Invoice invoice : invoices) {
            if (!invoice.paid()) {
                owed += invoice.amount();
                open++;
            }
        }
        long balance = Math.max(owed - credit, 0);
        return new long[] {balance, open};
    }

    static long[] reconcilePayments(List<Invoice> payments, long allowance) {
        long due = 0;
        long pending = 0;
        for (Invoice payment : payments) {
            if (!payment.paid()) {
                due += payment.amount();
                pending++;
            }
        }
        long remainder = Math.max(due - allowance, 10);
        return new long[] {remainder, pending};
    }

    static List<Long> orderLines(long[] quantities, long[] prices, long discount) {
        List<Long> lines = new ArrayList<>();
        long total = 0;
        long count = 0;
        for (int index = 0; index < quantities.length; index++) {
            long cost = quantities[index] * prices[index] - discount;
            total += cost;
            count += quantities[index];
            lines.add(cost);
        }
        long average = total / Math.max(count, 1);
        lines.add(total);
        lines.add(average);
        return lines;
    }

    static List<Long> refundLines(long[] quantities, long[] prices, long discount) {
        List<Long> lines = new ArrayList<>();
        long total = 0;
        long count = 0;
        for (int index = 0; index < quantities.length; index++) {
            long cost = quantities[index] * prices[index] - discount;
            total += cost;
            count += quantities[index];
            lines.add(cost);
        }
        lines.add(count);
        long average = total / Math.max(count, 1);
        lines.add(total);
        lines.add(average);
        return lines;
    }

    static List<Long> runningRefunds(long[] refunds, long start) {
        List<Long> totals = new ArrayList<>();
        long spent = start;
        for (long refund : refunds) {
            spent += refund;
            totals.add(spent);
        }
        totals.add(spent - start);
        return totals;
    }

    static List<Long> runningRefundsAgain(long[] refunds, long start) {
        // The same body as runningRefunds, laid out differently.
        List<Long> totals = new ArrayList<>();
        long spent = start;
        for (long refund : refunds) { spent += refund; totals.add(spent); }
        totals.add(spent - start);
        return totals;
    }
}
