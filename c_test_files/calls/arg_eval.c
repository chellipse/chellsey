/* Arguments are full expressions, evaluated (with their side effects) before
   the call. The locals mutated in the argument list must reach the callee. */
int diff(int a, int b) {
    return a - b;
}

int main(void) {
    int x = 10;
    return diff(x = x + 5, x); /* both args see x after the assignment: 15 - 15 = 0 */
}
