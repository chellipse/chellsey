/* The classic: comparing int with unsigned converts the int operand to
   unsigned (usual arithmetic conversions), so -1 compares as 4294967295 —
   *greater* than any small unsigned value. Balancing with `long` instead
   keeps -1 negative, because long represents every unsigned int. */
int main(void) {
    unsigned u = 1;
    int neg = -1;
    if (neg < u) { /* false! -1 converts to 4294967295 */
        return 1;
    }
    long l = 1;
    if (neg > l) { /* false: both convert to long, -1 stays -1 */
        return 2;
    }
    return 60;
}
