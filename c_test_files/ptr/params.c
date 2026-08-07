/* Pointers across calls: `T *` parameters and returns, and sub-`int`
   pointees — their slots hold the canonical form, so access through the
   pointer behaves exactly like access through the name. */
int bump(int *p, int by) {
    *p = *p + by;
    return *p;
}

int *pick(int *a, int *b, int which) {
    return which ? a : b;
}

int main(void) {
    int x = 30;
    if (bump(&x, 5) != 35) {
        return 1;
    }
    if (x != 35) {
        return 2;
    }
    int y = 100;
    *pick(&x, &y, 1) = 60; /* a call is a pointer-valued expression */
    if (x != 60 || y != 100) {
        return 3;
    }
    if (pick(&x, &y, 0) != &y) {
        return 4;
    }
    char c = 'a';
    char *pc = &c;
    *pc = *pc + 1; /* the store through `*pc` converts to `char` first */
    if (c != 'b') {
        return 5;
    }
    short s = -2;
    short *ps = &s;
    if (*ps + 3 != 1) { /* the loaded value promotes like a named `short` */
        return 6;
    }
    long l = 1;
    long *pl = &l;
    *pl = 4294967296L + 10;
    if (*pl - 4294967296L != x - 50) {
        return 7;
    }
    return 70;
}
