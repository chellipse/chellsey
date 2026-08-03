/* A goto may jump into a block, skipping a declaration's initializer; the
   variable is then in scope but uninitialized (here it is never read). */
int main(void) {
    int r = 0;
    goto inside;
    {
        int x = 999; /* initializer skipped by the jump */
    inside:
        r = 5;
    }
    return r; /* 5 */
}
