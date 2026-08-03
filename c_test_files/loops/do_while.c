/* A do-while sums 1..5; the body always runs at least once. */
int main(void) {
    int i = 1;
    int sum = 0;
    do {
        sum += i;
        i++;
    } while (i <= 5);
    return sum; /* 1+2+3+4+5 = 15 */
}
