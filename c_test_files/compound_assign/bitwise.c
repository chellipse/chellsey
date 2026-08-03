/* Bitwise compound assignments: &=, |=, ^= over an int mask. */
int main(void) {
    int x = 240;  /* 11110000 */
    x &= 60;      /* & 00111100 = 00110000 = 48 */
    x |= 3;       /* | 00000011 = 00110011 = 51 */
    x ^= 255;     /* ^ 11111111 = 11001100 = 204 */
    return x;     /* 204 */
}
