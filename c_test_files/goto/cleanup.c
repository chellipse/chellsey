/* The classic error-handling ladder: on failure, jump to a shared cleanup. */
int run(int fail_at) {
    int acquired = 0;
    if (fail_at == 1) {
        goto fail;
    }
    acquired += 1;
    if (fail_at == 2) {
        goto fail;
    }
    acquired += 10;
    return acquired; /* success: 11 */
fail:
    return -acquired; /* partial: 0 or -1 */
}

int main(void) {
    /* run(0)=11, run(1)=0, run(2)=-1 */
    return run(0) + run(1) - run(2); /* 11 + 0 - (-1) = 12 */
}
