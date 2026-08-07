/* Pointers, batch 1 (6.5.3.2): address-of and dereference on scalar locals.
   Every pointee here is a local variable — arrays, globals, and pointer
   arithmetic are later batches. */
int main(void) {
    int x = 41;
    int *p = &x;
    if (*p != 41) {
        return 1;
    }
    *p = *p + 1; /* write through the pointer, read back through the name */
    if (x != 42) {
        return 2;
    }
    x = 7; /* write through the name, read back through the pointer */
    if (*p != 7) {
        return 3;
    }
    if (p != &x) { /* same object, same address */
        return 4;
    }
    if (&*p != p) { /* `&*` cancels without an access (6.5.3.2p4) */
        return 5;
    }
    int *q = 0; /* a null pointer constant makes a null pointer */
    if (q) {
        return 6;
    }
    if (!p) { /* a real address tests true */
        return 7;
    }
    long a = (long)p; /* a pointer cast to an integer is its address value */
    if (a == 0) {
        return 8;
    }
    bool held = p; /* pointer-to-bool is the `!= 0` test, not a truncation */
    if (!held) {
        return 9;
    }
    q = p; /* pointer assignment: both now name the same object */
    *q = 70;
    return x;
}
