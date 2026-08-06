/* Unsigned arithmetic wraps modulo 2^32 by definition (6.2.5): 0 - 1 is
   4294967295, and adding 2 wraps 4294967295 + 2 to 1. */
int main(void) {
    unsigned x = 0;
    x = x - 1; /* 4294967295 */
    if (x != 4294967295u) {
        return 1;
    }
    unsigned y = x + 2; /* wraps to 1 */
    if (y != 1u) {
        return 2;
    }
    return 40 + y; /* 41 */
}
