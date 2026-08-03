/* The parameter behaves like any local: read in conditions, loop bounds,
   and expressions, and assignable. */
int main(int argc) {
    int n = 0;
    for (int i = 0; i < argc + 4; i = i + 1) {
        n = n + i; /* 0+1+2+3+4 = 10 */
    }
    if (argc == 1 && n == 10) {
        argc = argc + n; /* 11 */
    }
    return argc * 2; /* 22 */
}
