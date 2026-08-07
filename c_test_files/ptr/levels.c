/* Multiple indirection: a pointer is itself an object with an address. */
int main(void) {
    int x = 5;
    int *p = &x;
    int **pp = &p;
    if (**pp != 5) {
        return 1;
    }
    **pp = 6; /* through two levels down to `x` */
    if (x != 6) {
        return 2;
    }
    int y = 50;
    *pp = &y; /* through one level: repoint `p` at `y` */
    if (*p != 50) {
        return 3;
    }
    *&x = 20; /* `*&` also cancels back to the object */
    if (x != 20) {
        return 4;
    }
    bool alive = true;
    bool *pb = &alive;
    *pb = false;
    if (alive) {
        return 5;
    }
    if (*p + x == 70) {
        return 70;
    }
    return 6;
}
