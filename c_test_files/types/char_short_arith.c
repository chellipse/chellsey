/* char/short locals: a narrowing assignment re-extends per the target's width
   and signedness (6.3.1.3 — implementation-defined for a signed target; wraps
   two's-complement like gcc), and the integer promotions (6.3.1.1p2) make all
   arithmetic at least int-width. */
int main(void) {
    char c = 200; /* 200 doesn't fit signed char: wraps to -56 */
    if (c != -56) {
        return 1;
    }
    unsigned char uc = 300; /* 300 mod 256 = 44 */
    if (uc != 44) {
        return 2;
    }
    short s = 70000; /* 70000 - 65536 = 4464 */
    if (s != 4464) {
        return 3;
    }
    unsigned short us = 65535;
    us += 1; /* compound assign narrows back: wraps to 0 */
    if (us != 0) {
        return 4;
    }
    /* promotion: char operands widen to int, so no wrap at 8 bits */
    c = 100;
    int wide = c * 3; /* 300, not 300 & 0xFF */
    if (wide != 300) {
        return 5;
    }
    /* char and short mix under the usual arithmetic conversions */
    s = -5;
    if (c + s != 95) {
        return 6;
    }
    /* ++ narrows its result back into the operand's type */
    c = 127;
    c++; /* 128 converted back to char: -128 */
    if (c != -128) {
        return 7;
    }
    /* unsigned char stays zero-extended: 255 is 255, not -1 */
    uc = 255;
    if (uc < 0 || uc != 255) {
        return 8;
    }
    return 70;
}
