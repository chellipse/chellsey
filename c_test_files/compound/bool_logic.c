/* No && / || yet, so boolean logic is built from 0/1 comparison results with
   bitwise & (and), | (or), ^ (xor), and ! (not), then fed to if-conditions.
   Confirms comparison results compose correctly through the bitwise ops. */
int main(void) {
    if (!(((3 < 5) & (8 >= 8)) ^ (2 == 2))) { /* (1 & 1) ^ 1 = 0, !0 = 1 */
        if ((7 > 9) | (4 == 4)) {             /* 0 | 1 = 1 */
            return ((15 | 1) & 12) + (100 >> 2);
        }
        return 5;
    }
    return 9;
}
