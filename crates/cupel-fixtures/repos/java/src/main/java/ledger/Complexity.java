package ledger;

import java.util.List;

/** Functions over the complexity limits. */
public final class Complexity {
    private Complexity() {
    }

    /** Names an amount by the first rule it matches. */
    public static String label(long amount, long limit) {
        String label = "";
        if (amount < 0) {
            label = "refund";
        } else if (amount == 0) {
            label = "empty";
        } else if (amount > limit * 100) {
            label = "huge";
        } else if (amount > limit * 10) {
            label = "large";
        } else if (amount > limit) {
            label = "over";
        } else if (amount == limit) {
            label = "at-limit";
        } else if (amount > limit / 2) {
            label = "high";
        } else if (amount > limit / 4) {
            label = "medium";
        } else if (amount > limit / 10) {
            label = "low";
        } else if (amount > 1) {
            label = "tiny";
        } else {
            label = "unit";
        }
        return label;
    }

    /** Applies positive amounts of non-empty batches until the limit is reached. */
    public static long settle(long[][] batches, long limit) {
        long settled = 0;
        for (long[] batch : batches) {
            if (batch.length > 0) {
                for (long amount : batch) {
                    if (amount > 0) {
                        while (settled < limit) {
                            settled += amount;
                        }
                    }
                }
            }
        }
        return settled;
    }

    /** Describes a new account. */
    public static String openAccount(String owner, String currency, long limit, boolean overdraft, int branch, long opened) {
        return String.format("%s/%s/%d/%b/%d/%d", owner, currency, limit, overdraft, branch, opened);
    }

    /** The fixed lines of a printed statement. */
    public static List<String> statementTemplate() {
        return List.of(
            "ACME LEDGER",
            "===========",
            "",
            "Account holder:",
            "Account number:",
            "Branch:",
            "Currency:",
            "Statement period:",
            "",
            "Opening balance",
            "---------------",
            "Balance brought forward:",
            "Pending credits:",
            "Pending debits:",
            "",
            "Credits",
            "-------",
            "Deposits:",
            "Transfers in:",
            "Interest:",
            "Refunds:",
            "Adjustments:",
            "",
            "Debits",
            "------",
            "Withdrawals:",
            "Transfers out:",
            "Card payments:",
            "Direct debits:",
            "Standing orders:",
            "Fees:",
            "Charges:",
            "",
            "Closing balance",
            "---------------",
            "Balance carried forward:",
            "Available balance:",
            "Overdraft limit:",
            "Overdraft used:",
            "",
            "Notices",
            "-------",
            "Rates changed this period.",
            "Fees are listed above.",
            "Keep this statement for your records.",
            "Report errors within thirty days.",
            "",
            "Contact",
            "-------",
            "Telephone:",
            "Email:",
            "Post:",
            "",
            "Opening hours",
            "-------------",
            "Weekdays: 9 to 5",
            "Saturday: 9 to 12",
            "Sunday: closed",
            "",
            "Page 1 of 1",
            "End of statement");
    }

    /** The difference between credits and debits. */
    public static long net(long credits, long debits) {
        return credits - debits;
    }

    /** Names a month by its number: one switch, however many cases it has. */
    public static String month(int number) {
        switch (number) {
            case 1:
                return "jan";
            case 2:
                return "feb";
            case 3:
                return "mar";
            case 4:
                return "apr";
            case 5:
                return "may";
            case 6:
                return "jun";
            case 7:
                return "jul";
            case 8:
                return "aug";
            case 9:
                return "sep";
            case 10:
                return "oct";
            case 11:
                return "nov";
            case 12:
                return "dec";
            default:
                return "none";
        }
    }

    /** A printed statement for one account. */
    public static final class Statement {
        private final String owner;

        /** A statement for owner. */
        public Statement(String owner) {
            this.owner = owner;
        }

        /** One line of the statement. */
        public String line(long amount) {
            return owner + ": " + amount;
        }
    }
}
