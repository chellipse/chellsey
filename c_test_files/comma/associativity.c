/* The comma operator is left-associative; a chain's value is its last
   operand, and every operand is evaluated in order. */
int main(void) {
    int t = 0;
    int last = (t = t + 1, t = t + 10, t = t + 100); /* t: 1 -> 11 -> 111 */
    return last;                                     /* 111 */
}
