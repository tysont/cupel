package discount;

import static org.junit.jupiter.api.Assertions.assertEquals;

import org.junit.jupiter.api.Test;

/** A large member order: the discount applies. */
class LargeTest {
    @Test
    void discountsALargeMemberOrder() {
        assertEquals(190, Discount.discount(200, true));
    }
}
