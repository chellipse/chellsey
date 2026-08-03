/* goto forming a loop (retry/done) around a switch and a helper call. */
int parity(int n) {
    return n % 2;
}

int main(void) {
    int evens = 0;
    int odds = 0;
    int i = 0;
retry:
    if (i >= 6) {
        goto done;
    }
    switch (parity(i)) {
        case 0:
            evens++;
            break;
        default:
            odds++;
    }
    i++;
    goto retry;
done:
    return evens * 10 + odds; /* i=0..5: evens=3, odds=3 -> 33 */
}
