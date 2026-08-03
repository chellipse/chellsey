/* A forward goto skips the code between the jump and its label. */
int main(void) {
    int x = 1;
    goto skip;
    x = 100; /* unreachable: skipped by the goto */
skip:
    return x; /* 1 */
}
