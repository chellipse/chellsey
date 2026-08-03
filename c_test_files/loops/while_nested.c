/* Nested loops: 6 * 7 by repeated addition, with a body-scoped counter. */
int main(void) {
    int product = 0;
    int i = 0;
    while (i < 6) {
        int j = 0;
        while (j < 7) {
            product = product + 1;
            j = j + 1;
        }
        i = i + 1;
    }
    return product; /* 42 */
}
