/* Prefix and postfix differ by exactly the value handed back. */
int main(void) {
    int i = 3;
    int pre = ++i;   /* i=4, pre=4 */
    int j = 3;
    int post = j++;  /* post=3, j=4 */
    return (pre - post) * 100 + i * 10 + j; /* 100 + 40 + 4 = 144 */
}
