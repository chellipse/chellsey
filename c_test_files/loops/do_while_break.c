/* `break` leaves a do-while immediately. */
int main(void) {
    int i = 0;
    int sum = 0;
    do {
        if (i == 3) {
            break;
        }
        sum += i;
        i++;
    } while (i < 10);
    return sum; /* 0+1+2 = 3 */
}
