/* Compound assignment is right-associative and yields the stored value, so it
   composes: `a += b += c` is `a += (b += c)`. */
int main(void) {
    int a = 1;
    int b = 2;
    int c = 3;
    a += b += c;       /* b becomes 5, then a becomes 6 */
    return a * 10 + b; /* 6*10 + 5 = 65 */
}
