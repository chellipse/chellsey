// ME-8 batch 2: read-modify-write through a pointer — `*p op= n` for every
// compound operator, `(*p)++`/`--*p`, and subscript targets, at several
// pointee widths. The address is computed exactly once (6.5.16.2p3), which
// the side-effecting subscript checks. Returns 70.

int add_via(int *p, int n) {
    *p += n;
    return *p;
}

int main(void) {
    int x = 10;
    int *p = &x;

    *p += 5;
    *p -= 3;
    *p *= 4;
    *p /= 2;
    *p %= 14;
    *p <<= 2;
    *p >>= 1;
    *p |= 5;
    *p &= 28;
    *p ^= 3;
    if (x != 23) return 1;

    // ++/-- through the pointer: postfix yields the old value, prefix the new
    if ((*p)++ != 23) return 2;
    if (x != 24) return 3;
    if (++*p != 25) return 4;
    if ((*p)-- != 25) return 5;
    if (--*p != 23) return 6;
    if (x != 23) return 7;

    // subscript targets desugar to the same deref
    p[0] += 7;
    p[0]--;
    if (x != 29) return 8;

    // narrow pointees: the result converts back to the lvalue's type on the
    // store (6.5.16.2), wrapping/truncating like a direct assignment
    char c = 100;
    char *pc = &c;
    *pc += 100;
    if (c != -56) return 9;
    (*pc)++;
    if (c != -55) return 10;

    unsigned short us = 65535;
    unsigned short *pus = &us;
    *pus += 1;
    if (us != 0) return 11;
    (*pus)--;
    if (us != 65535) return 12;

    long big = 1;
    long *pl = &big;
    *pl <<= 40;
    if (*pl >> 40 != 1) return 13;
    if (big != 1099511627776) return 14;

    bool t = true;
    bool *pt = &t;
    *pt ^= 1;
    if (t) return 15;
    *pt += 1;
    if (!t) return 16;

    // a side-effecting subscript: the address is computed once, before the
    // read-modify-write, with the old index value
    x = 50;
    int i = 0;
    p[i++] += 12;
    if (i != 1) return 17;
    if (x != 62) return 18;

    x = 63;
    return add_via(p, 7);
}
