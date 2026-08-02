/* Arithmetic binds tighter than comparison: (2 + 3) < (4 * 2) = 5 < 8 = 1. */
int main(void) { return 2 + 3 < 4 * 2; }
