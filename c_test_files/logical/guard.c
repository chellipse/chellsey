/* The canonical guard: the division only ever runs because the lhs
   short-circuits it away when b is 0 — a broken `&&` divides by zero. */
int main(void) {
    int acc = 0;
    for (int b = -3; b <= 3; b = b + 1) {
        if (b != 0 && 12 / b > 2) {
            acc = acc + 1;
        }
    }
    return acc; /* b in {1, 2, 3}: 12, 6, 4 all > 2 -> 3 */
}
