/* `break` leaves an otherwise-infinite loop. */
int main(void) {
    int n = 1;
    while (1) {
        if (n * n > 50) {
            break;
        }
        n = n + 1;
    }
    return n; /* 8 */
}
