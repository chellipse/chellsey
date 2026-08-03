/* Iterate the Fibonacci recurrence: fib(13) = 233. */
int main(void) {
    int a = 0;
    int b = 1;
    int n = 13;
    while (n > 1) {
        int t = a + b;
        a = b;
        b = t;
        n = n - 1;
    }
    return b;
}
