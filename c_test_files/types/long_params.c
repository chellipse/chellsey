/* `long` parameters and returns travel as full 8-byte values through the
   SysV registers; the sum only fits in 64 bits. */
long scale(long base, int by) {
    return base * by;
}

int main(void) {
    long r = scale(1000000000L, 6); /* 6e9 */
    return r / 100000000L;          /* 60 */
}
