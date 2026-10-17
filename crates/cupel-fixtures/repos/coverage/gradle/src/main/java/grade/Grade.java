package grade;

/** A suite that covers total fully, classify partly, and refund not at all. */
public final class Grade {
    private Grade() {
    }

    /** The sum of values. */
    public static int total(int[] values) {
        int sum = 0;
        for (int value : values) {
            sum += value;
        }
        return sum;
    }

    /** A letter for score; the tests never reach the failing grade. */
    public static String classify(int score) {
        if (score >= 90) {
            return "A";
        }
        if (score >= 50) {
            return "pass";
        }
        return "fail";
    }

    /** What to refund of amount after days; no test calls it. */
    public static int refund(int amount, int days, boolean member) {
        if (amount <= 0) {
            return 0;
        }
        if (days > 30) {
            return 0;
        }
        if (member && days < 7) {
            return amount;
        }
        if (member) {
            return amount / 2;
        }
        return amount / 4;
    }
}
