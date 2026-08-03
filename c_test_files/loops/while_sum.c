/* Sum 1..10 with a condition-controlled loop. */
int main(void) {
    int sum = 0;
    int i = 1;
    while (i <= 10) {
        sum = sum + i;
        i = i + 1;
    }
    return sum; /* 55 */
}
