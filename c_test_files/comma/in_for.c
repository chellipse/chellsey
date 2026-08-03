/* Comma in the for-init and for-step expressions: the classic two-variable
   loop, i rising and j falling. */
int main(void) {
    int i;
    int j;
    int sum = 0;
    for (i = 0, j = 10; i < j; i++, j--) {
        sum += 1;
    }
    /* (0,10)(1,9)(2,8)(3,7)(4,6) then i=5,j=5 stops -> 5 iterations */
    return sum; /* 5 */
}
