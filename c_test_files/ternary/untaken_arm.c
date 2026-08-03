/* Only the taken arm evaluates (6.5.15): the untaken assignments must not
   fire. */
int main(void) {
    int x = 0;
    int r1 = 1 ? 5 : (x = 9);
    int r2 = 0 ? (x = 4) : 6;
    if (x != 0) {
        return 100; /* an untaken arm ran */
    }
    return r1 * 10 + r2; /* 56 */
}
