/* A label can live in code unreachable by fallthrough yet still be a live goto
   target — the code after the `return` is reached only via `goto zero`. */
int pick(int n) {
    if (n == 0) {
        goto zero;
    }
    return n * 2;
zero:
    return 42;
}

int main(void) {
    return pick(0) - pick(3); /* 42 - 6 = 36 */
}
