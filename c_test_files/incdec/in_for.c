/* The idiomatic counters: `i++` in a for-step, `n--` in a loop condition. */
int main(void) {
    int sum = 0;
    for (int i = 0; i < 5; i++) {
        sum += i;          /* 0+1+2+3+4 = 10 */
    }
    int n = 10;
    while (n-- > 0) {       /* body runs 10 times; n ends at -1 */
        sum += 1;
    }
    return sum;            /* 10 + 10 = 20 */
}
