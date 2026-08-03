/* Compound assignments woven through calls, a loop, and a branch condition. */
int step(int n) {
    return n + 1;
}

int main(void) {
    int acc = 0;
    int i = 0;
    while (i < 4) {
        acc += step(i) * 2; /* +2, +4, +6, +8 -> 20 */
        i += 1;
    }
    acc *= 2;   /* 40 */
    acc -= 5;   /* 35 */
    acc %= 30;  /* 5 */
    return acc; /* 5 */
}
