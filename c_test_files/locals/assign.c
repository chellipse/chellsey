/* An uninitialized declaration, assigned before its first read. */
int main(void) {
    int x;
    x = 7;
    return x;
}
