/* Mixed int/long arithmetic balances to `long` (usual arithmetic
   conversions): the int operand widens, so the multiply cannot wrap at 32
   bits. */
int main(void) {
    int m = 100000;
    long r = m * 50000L; /* 5e9, exact in long */
    if (r != 5000000000L) {
        return 1;
    }
    /* comparison balances to long too: 5e9 does not compare as its low bits */
    if (r < 1000000) {
        return 2;
    }
    return 30;
}
