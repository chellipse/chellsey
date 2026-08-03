/* Calls as arguments to calls: each result has its own slot, so evaluating
   one argument cannot clobber another. */
int add(int a, int b) {
    return a + b;
}

int mul(int a, int b) {
    return a * b;
}

int main(void) {
    return add(mul(3, 4), mul(add(1, 1), 5)); /* 12 + 10 = 22 */
}
