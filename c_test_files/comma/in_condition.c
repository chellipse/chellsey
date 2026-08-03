/* A comma operator inside a controlling expression: the left runs for effect,
   the right is what the `if` tests. */
int main(void) {
    int seen = 0;
    int x = 0;
    if (seen = 1, x == 0) {
        return seen * 10 + 7; /* seen=1 -> 17 */
    }
    return seen;
}
