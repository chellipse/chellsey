/* A call to a defined function, result used in an expression. */
int add(int a, int b) {
    return a + b;
}

int main(void) {
    return add(40, 2); /* 42 */
}
