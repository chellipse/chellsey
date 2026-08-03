/* Prefix ++/-- modify in place and yield the *new* value. */
int main(void) {
    int x = 5;
    int a = ++x;           /* x=6, a=6 */
    int b = --x;           /* x=5, b=5 */
    return a * 10 + b + x; /* 60 + 5 + 5 = 70 */
}
