/* `=` is right-associative and yields the stored value (6.5.16.1). */
int main(void) {
    int x;
    int y;
    x = y = 6;
    return x + y; /* 12 */
}
