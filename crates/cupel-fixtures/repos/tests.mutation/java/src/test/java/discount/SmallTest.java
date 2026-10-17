package discount;

import static org.junit.jupiter.api.Assertions.assertEquals;

import org.junit.jupiter.api.Test;

/** A small order: no discount; never the boundary of exactly 100. */
class SmallTest {
    @Test
    void keepsASmallOrder() {
        assertEquals(50, Discount.discount(50, true));
    }
}
