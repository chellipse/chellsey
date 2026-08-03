/* `continue` in a do-while jumps to the controlling expression (6.8.6.2), so
   the loop keeps running rather than exiting. */
int main(void) {
    int i = 0;
    int sum = 0;
    do {
        i++;
        if (i == 3) {
            continue;  /* skip adding 3, but still test i < 6 */
        }
        sum += i;
    } while (i < 6);
    return sum; /* 1+2+4+5+6 = 18 */
}
