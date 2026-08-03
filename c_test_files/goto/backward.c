/* A backward goto builds a loop by hand: sum 1..5. */
int main(void) {
    int i = 1;
    int sum = 0;
loop:
    sum += i;
    i++;
    if (i <= 5) {
        goto loop;
    }
    return sum; /* 15 */
}
