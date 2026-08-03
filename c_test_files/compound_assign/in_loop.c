/* Compound assignment as the idiomatic loop accumulator — including `+=` in
   the for-step position. */
int main(void) {
    int sum = 0;
    int prod = 1;
    for (int i = 1; i <= 5; i += 1) {
        sum += i;   /* 1+2+3+4+5 = 15 */
        prod *= i;  /* 5! = 120 */
    }
    return sum + prod; /* 135 */
}
