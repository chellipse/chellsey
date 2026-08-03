/* Declarations inside branch arms, updating an outer variable. */
int main(void) {
    int v = 10;
    if (v > 5) {
        int w = v * 2;
        v = w + 1; /* 21 */
    } else {
        int w = v - 2;
        v = w;
    }
    return v;
}
