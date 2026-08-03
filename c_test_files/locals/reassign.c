/* Reassignment reading the variable's previous value. */
int main(void) {
    int x = 1;
    x = x + 1;  /* 2 */
    x = x * 10; /* 20 */
    return x;
}
