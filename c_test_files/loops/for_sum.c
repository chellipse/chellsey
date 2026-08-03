/* A counted for loop with a clause-scoped counter. */
int main(void) {
    int sum = 0;
    for (int i = 1; i <= 10; i = i + 1) {
        sum = sum + i;
    }
    return sum; /* 55 */
}
