/* Boolean logic two ways: the original bitwise combinations of 0/1
   comparison results, now joined by the real short-circuit && / || over the
   same predicates (with runtime division guards only short-circuiting can
   make safe). */
int main(void) {
    if (!(((3 < 5) & (8 >= 8)) ^ (2 == 2))) { /* (1 & 1) ^ 1 = 0, !0 = 1 */
        if ((7 > 9) | (4 == 4)) {             /* 0 | 1 = 1 */
            int z = 0;
            int bitwise = ((15 | 1) & 12) + (100 >> 2); /* 12 + 25 = 37 */
            int logical = (3 < 5 && 8 >= 8) /* 1 */
                + (7 > 9 || 4 == 4)         /* 1 */
                + (z && 12 / z)             /* 0, the division never runs */
                + (z == 0 || 12 / z);       /* 1, likewise */
            return bitwise + logical * 10; /* 37 + 30 = 67 */
        }
        return 5;
    }
    return 9;
}
