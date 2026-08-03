/* Locals as operands throughout the expression tower. */
int main(void) {
    int a = 6;
    int b = 7;
    int c = a << 1;                    /* 12 */
    return a * b - (a & b) + (c > b); /* 42 - 6 + 1 = 37 */
}
