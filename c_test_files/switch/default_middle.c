/* `default` need not be last, and cases may appear in any order. */
int pick(int n) {
    int r = 0;
    switch (n) {
        case 3:
            r = 33;
            break;
        default:
            r = 99;
            break;
        case 1:
            r = 11;
            break;
    }
    return r;
}

int main(void) {
    return pick(1) + pick(3) + pick(7) - 100;
    /* 11 + 33 + 99 - 100 = 43 */
}
