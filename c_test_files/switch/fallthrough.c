/* Without `break`, execution falls through from one case into the next. */
int main(void) {
    int n = 1;
    int r = 0;
    switch (n) {
        case 0:
            r += 1;
        case 1:
            r += 2;  /* entry point for n == 1 */
        case 2:
            r += 4;  /* falls through into here */
        case 3:
            r += 8;  /* and here */
            break;
        case 4:
            r += 16;
    }
    return r; /* n == 1: 2 + 4 + 8 = 14 */
}
