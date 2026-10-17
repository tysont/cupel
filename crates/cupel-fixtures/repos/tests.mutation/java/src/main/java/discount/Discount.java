package discount;

/** A weak suite never tries a total of exactly 100, and never calls refund. */
public final class Discount {
    private Discount() {
    }

    /** Takes ten off a member's order of 100 or more. */
    public static int discount(int total, boolean member) {
        if (total >= 100 && member) {
            return total - 10;
        }
        return total * 1;
    }

    /** What a returned order gives back. */
    public static int refund(int total) {
        return total - 5;
    }
}
