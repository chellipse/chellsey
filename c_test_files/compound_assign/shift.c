/* Shift compound assignments; >>= on a signed int is an arithmetic shift. */
int main(void) {
    int x = 3;
    x <<= 4;      /* 48 */
    x >>= 1;      /* 24 */
    int y = -32;
    y >>= 2;      /* -8, sign preserved */
    return x + y; /* 24 + (-8) = 16 */
}
