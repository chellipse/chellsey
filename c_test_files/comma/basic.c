/* The comma operator evaluates both sides left to right; its value is the
   right operand (6.5.17). */
int main(void) {
    int a = 0;
    int x = (a = 5, a + 1); /* a becomes 5, the value is 6 */
    return x + a;           /* 6 + 5 = 11 */
}
