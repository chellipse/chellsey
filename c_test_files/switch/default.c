/* `default` runs when no case matches. */
int describe(int n) {
    int r = 0;
    switch (n) {
        case 1:
            r = 1;
            break;
        case 2:
            r = 2;
            break;
        default:
            r = 9;
            break;
    }
    return r;
}

int main(void) {
    return describe(1) * 100 + describe(2) * 10 + describe(5);
    /* 100 + 20 + 9 = 129 */
}
