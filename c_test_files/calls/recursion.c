/* Self-recursion: factorial(5) = 120. Exercises the call stack and the
   spill-everything frame surviving nested activations. */
int fact(int n) {
    if (n <= 1) {
        return 1;
    }
    return n * fact(n - 1);
}

int main(void) {
    return fact(5); /* 120 */
}
