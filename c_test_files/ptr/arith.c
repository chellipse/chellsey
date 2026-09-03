// ME-8 batch 2: element-scaled pointer arithmetic and subscripts. Every
// observable value is an offset between pointers sharing a base, so the
// result is layout-independent. Returns 70.

int main(void) {
    int x = 5;
    int *p = &x;

    // byte scaling, observed through exact ptr->int casts: +1 element
    // moves 4 bytes for int*, 1 for char*, 8 for long* and int**
    long pi = (long)(p + 1) - (long)p;
    char c = 'a';
    char *pc = &c;
    long pcs = (long)(pc + 1) - (long)pc;
    long l = 7;
    long *pl = &l;
    long pls = (long)(pl + 1) - (long)pl;
    int **pp = &p;
    long pps = (long)(pp + 1) - (long)pp;
    if (pi != 4 || pcs != 1 || pls != 8 || pps != 8) return 1;

    // n + p commutes; - undoes +; the difference divides back to elements
    int *q = 3 + p;
    if (q - p != 3) return 2;
    if (p - q != -3) return 3;
    if (q - 3 != p) return 4;
    if ((q - 2) - (p + 1) != 0) return 5;

    // negative and unsigned indexes: the canonical form is the value
    int back = -2;
    if (q + back != p + 1) return 6;
    unsigned int ui = 2u;
    if ((p + ui) - p != 2) return 7;
    long li = 2;
    if ((p + li) - p != 2) return 8;

    // subscripting is *(p + i): reads, writes, i[p], &p[i]
    if (p[0] != 5) return 9;
    p[0] = 11;
    if (x != 11) return 10;
    if (0[p] != 11) return 11;
    if (&p[3] - p != 3) return 12;
    if (&3[p] != p + 3) return 13;

    // ++/-- step by one element, prefix and postfix
    int *r = p;
    r++;
    if (r - p != 1) return 14;
    if (r-- != p + 1) return 15;
    if (r != p) return 16;
    if (++r != p + 1) return 17;
    if (--r != p) return 18;

    // += / -= step by elements and yield the moved pointer
    r += 3;
    if (r - p != 3) return 19;
    if ((r -= 2) != p + 1) return 20;
    r -= 1;
    if (r != p) return 21;

    // the moved-and-restored pointer still reaches the object
    *r = 70;
    return x;
}
