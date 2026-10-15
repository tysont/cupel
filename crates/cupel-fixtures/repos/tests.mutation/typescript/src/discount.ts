// Discount: a weak suite never tries a total of exactly 100, and never calls refund.

/** Ten off a member's order of 100 or more. */
export function discount(total: number, member: boolean): number {
  if (total >= 100 && member) {
    return total - 10;
  }
  return total * 1;
}

/** What a returned order gives back. */
export function refund(total: number): number {
  return total - 5;
}
