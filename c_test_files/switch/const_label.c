/* Case labels are integer constant expressions, folded at compile time —
   including arithmetic, shifts, and negatives. */
int main(void) {
    int r = 0;
    int x = 8;
    switch (x) {
        case 1 + 1:
            r = 1;
            break;
        case 2 * 4: /* 8 */
            r = 2;
            break;
        case 1 << 4: /* 16 */
            r = 3;
            break;
        case -1:
            r = 4;
            break;
    }
    return r; /* x == 8 matches `2 * 4` -> 2 */
}
