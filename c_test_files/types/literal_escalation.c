/* 6.4.4.1 magnitude escalation: an unsuffixed constant too large for `int`
   moves to the next type that fits. A decimal constant escalates through the
   signed types only (int -> long), so 2^31 is a positive `long`, not INT_MIN;
   a hex constant also admits the unsigned types, so 0xFFFFFFFF is `unsigned
   int`, not a negative `int`. Getting the type wrong would truncate or
   sign-flip these at the 32-bit boundary. */
int main(void) {
    /* decimal 2^31 is `long`: stays positive rather than wrapping to INT_MIN */
    if (2147483648 <= 0) {
        return 1;
    }
    /* and participates in 64-bit arithmetic without wrapping at 32 bits */
    long sum = 2147483648 + 2147483648; /* 2^32 */
    if (sum != 4294967296L) {
        return 2;
    }
    /* hex 0xFFFFFFFF is `unsigned int` (2^32 - 1), not int -1 */
    if (0xFFFFFFFF <= 0) {
        return 3;
    }
    /* hex past unsigned int escalates to `long`, keeping full magnitude */
    if (0x100000000 != 4294967296L) {
        return 4;
    }
    return 70;
}
