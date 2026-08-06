/* Converting long -> int keeps the low 32 bits, sign-extended: 0x1_0000_002A
   truncates to 42, and 0xFFFF_FFFF truncates to -1. */
int main(void) {
    long big = 4294967338L; /* 2^32 + 42 */
    int t = big;
    long m1 = 4294967295L; /* 2^32 - 1 */
    int n = m1;
    if (n != -1) {
        return 1;
    }
    return t; /* 42 */
}
