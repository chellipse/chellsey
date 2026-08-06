/* `long` is 8 bytes: arithmetic that overflows an `int` is exact in `long`.
   3000000000 + 3000000000 = 6000000000; dividing back down recovers a small
   quotient an exit code can carry. */
int main(void) {
    long a = 3000000000L;
    long b = a + a;
    return b / 100000000L; /* 60 */
}
