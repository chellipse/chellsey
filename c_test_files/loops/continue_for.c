/* `continue` in a for loop must still run the step (6.8.6.2), or this
   would never terminate. */
int main(void) {
    int sum = 0;
    for (int i = 1; i <= 10; i = i + 1) {
        if ((i & 1) == 0) {
            continue; /* skip the evens */
        }
        sum = sum + i;
    }
    return sum; /* 1+3+5+7+9 = 25 */
}
