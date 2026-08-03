/* An empty init clause reusing an outer variable; the step multiplies. */
int main(void) {
    int i = 5;
    for (; i < 40; i = i * 2) {
        ;
    }
    return i; /* 5, 10, 20, 40 -> 40 */
}
