/* Postfix ++/-- modify in place but yield the *old* value. */
int main(void) {
    int x = 5;
    int a = x++;           /* a=5, x=6 */
    int b = x--;           /* b=6, x=5 */
    return a * 10 + b + x; /* 50 + 6 + 5 = 61 */
}
