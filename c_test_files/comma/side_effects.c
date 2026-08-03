/* Each operand's side effects happen in order — there is a sequence point
   between them. */
int main(void) {
    int n = 1;
    int r = (n = n * 10, n = n + 2, n); /* n: 1 -> 10 -> 12; value 12 */
    return r;                           /* 12 */
}
