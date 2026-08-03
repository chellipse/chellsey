/* Mutual recursion through a forward prototype: is 17 odd? */
int is_even(int n);

int is_odd(int n) {
    if (n == 0) {
        return 0;
    }
    return is_even(n - 1);
}

int is_even(int n) {
    if (n == 0) {
        return 1;
    }
    return is_odd(n - 1);
}

int main(void) {
    return is_odd(17) * 10 + is_even(17); /* 10 + 0 = 10 */
}
