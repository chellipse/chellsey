/* char/short across the call ABI: an incoming sub-int argument's upper bits
   are unspecified (SysV), so the callee re-extends each per its own type; a
   sub-int return value is converted from the full-width expression and
   travels back canonically. */
short add3(short a, short b, short c) {
    return a + b + c; /* computed as int; the return narrows it */
}

unsigned char wrap(unsigned char x, int n) {
    return x + n;
}

char keep_sign(char x) {
    return x;
}

int main(void) {
    if (add3(30000, 30000, 30000) != 24464) { /* 90000 wraps at 16 bits */
        return 1;
    }
    if (wrap(200, 100) != 44) { /* 300 mod 256 */
        return 2;
    }
    if (keep_sign(-1) != -1) { /* sign survives the boundary */
        return 3;
    }
    if (wrap(255, 1) + add3(1, 2, 3) != 6) { /* 0 + 6 */
        return 4;
    }
    return 70;
}
