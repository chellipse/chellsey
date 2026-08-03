/* `for (;;)` with every clause absent, terminated from inside. */
int main(void) {
    int n = 0;
    for (;;) {
        n = n + 1;
        if (n == 7) {
            return n;
        }
    }
}
