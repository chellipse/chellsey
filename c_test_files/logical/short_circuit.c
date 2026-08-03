/* When the lhs decides, the rhs must not evaluate: the assignments in the
   rhs must never fire. */
int main(void) {
    int x = 0;
    int r1 = 0 && (x = 1);
    int r2 = 1 || (x = 2);
    if (x != 0) {
        return 100; /* a rhs ran that should not have */
    }
    return r1 * 10 + r2; /* 0*10 + 1 = 1 */
}
