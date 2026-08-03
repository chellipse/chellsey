/* The value of a compound assignment is the new value of the lvalue, usable
   directly inside a larger expression. */
int main(void) {
    int x = 4;
    int y = (x += 6) * 2; /* x becomes 10; y = 10 * 2 = 20 */
    return x + y;         /* 10 + 20 = 30 */
}
