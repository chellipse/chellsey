/* A switch dispatching among helper calls on a computed discriminant, inside a
   loop. */
int sq(int n) {
    return n * n;
}
int dbl(int n) {
    return n + n;
}

int main(void) {
    int total = 0;
    for (int i = 1; i <= 4; i++) {
        switch (i % 3) {
            case 0:
                total += sq(i); /* i=3 -> 9 */
                break;
            case 1:
                total += dbl(i); /* i=1 -> 2, i=4 -> 8 */
                break;
            default:
                total += i; /* i=2 -> 2 */
        }
    }
    return total; /* 2 + 2 + 9 + 8 = 21 */
}
