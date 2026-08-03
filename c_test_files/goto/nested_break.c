/* goto is the clean way out of nested loops — something `break` can't do in a
   single hop. */
int main(void) {
    int found = 0;
    int i = 0;
    int j = 0;
    for (i = 0; i < 5; i++) {
        for (j = 0; j < 5; j++) {
            if (i * 5 + j == 13) {
                found = 1;
                goto done;
            }
        }
    }
done:
    return found * 100 + i * 10 + j; /* i=2, j=3 -> 100 + 20 + 3 = 123 */
}
