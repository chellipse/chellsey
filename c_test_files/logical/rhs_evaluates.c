/* When the lhs does not decide, the rhs runs — side effects included. */
int main(void) {
    int x = 0;
    int y = 0;
    int r1 = 1 && (x = 7); /* rhs runs: x = 7, r1 = 1 */
    int r2 = 0 || (y = 9); /* rhs runs: y = 9, r2 = 1 */
    return r1 * 100 + r2 * 10 + (x + y - 16); /* 100 + 10 + 0 = 110 */
}
