/* Increment/decrement in loop conditions: a count-up and a count-down. */
int main(void) {
    int count = 0;
    int i = 0;
    while (i++ < 4) {    /* i is 1,2,3,4 in the body -> 1+2+3+4 = 10 */
        count += i;
    }
    int down = 0;
    int j = 3;
    while (--j >= 0) {   /* j is 2,1,0 in the body -> 2+1+0 = 3 */
        down += j;
    }
    return count + down; /* 10 + 3 = 13 */
}
