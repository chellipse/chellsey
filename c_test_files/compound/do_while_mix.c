/* A do-while driving a small iteration with a helper call. */
int next(int x) {
    return x * 2 + 1;
}

int main(void) {
    int x = 1;
    int steps = 0;
    do {
        x = next(x);
        steps++;
    } while (x < 50);
    /* x: 1 -> 3 -> 7 -> 15 -> 31 -> 63 (>=50, stop). steps = 5, x = 63 */
    return steps * 10 + (x - 63); /* 50 + 0 = 50 */
}
