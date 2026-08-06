/* Division and `>>` are the operators whose machine form depends on
   signedness: 0xFFFFFFFE as unsigned divides to ~2^31, but the same bits as
   int would divide to -1; `>>` of a high-bit value shifts in zeros. */
int main(void) {
    unsigned big = 4294967294u; /* 0xFFFFFFFE */
    unsigned q = big / 2;       /* 2147483647, not -1 */
    if (q != 2147483647u) {
        return 1;
    }
    if (big % 1000000000u != 294967294u) {
        return 2;
    }
    unsigned h = 2147483648u; /* 0x80000000 */
    if (h >> 31 != 1u) {      /* logical shift: 1, not -1 */
        return 3;
    }
    int sh = -8;
    if (sh >> 1 != -4) { /* arithmetic shift for the signed type */
        return 4;
    }
    return 50;
}
