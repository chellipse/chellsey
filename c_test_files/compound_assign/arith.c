/* The arithmetic compound assignments, each modifying a local in place. */
int main(void) {
    int x = 100;
    x += 5;   /* 105 */
    x -= 20;  /* 85 */
    x *= 3;   /* 255 */
    x /= 4;   /* 63 */
    x %= 10;  /* 3 */
    return x; /* 3 */
}
