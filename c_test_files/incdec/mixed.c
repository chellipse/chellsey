/* ++/-- composing with arithmetic and a call — each statement modifies the
   counter exactly once, so it's well-defined. */
int twice(int n) {
    return n + n;
}

int main(void) {
    int x = 1;
    int total = 0;
    total += twice(x++);  /* twice(1)=2; x=2; total=2 */
    total += twice(++x);  /* x=3; twice(3)=6; total=8 */
    total -= --x;         /* x=2; total=8-2=6 */
    return total;         /* 6 */
}
