/* Loops woven through locals, branches, and the operator tower. */
int main(void) {
    int total = 0;
    int i = 1;
    while (i <= 10) {
        if ((i & 1) == 1) {
            total = total + i * i; /* odd: add its square */
        } else {
            total = total - i; /* even: subtract it */
        }
        i = i + 1;
    }
    /* odd squares 1+9+25+49+81 = 165, evens 2+4+6+8+10 = 30 -> 135 */
    int scale = 1;
    while (scale < total) {
        scale = scale << 1; /* first power of two >= 135: 256 */
    }
    return total + (scale == 256) - 100; /* 135 + 1 - 100 = 36 */
}
