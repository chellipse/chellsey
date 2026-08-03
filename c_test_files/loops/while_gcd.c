/* Euclid by repeated subtraction: gcd(252, 105) = 21. */
int main(void) {
    int a = 252;
    int b = 105;
    while (a != b) {
        if (a > b) {
            a = a - b;
        } else {
            b = b - a;
        }
    }
    return a;
}
