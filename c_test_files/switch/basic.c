/* A switch dispatches on the discriminant; each case breaks out. A value with
   no matching case and no default leaves the result untouched. */
int classify(int n) {
    int r = 0;
    switch (n) {
        case 0:
            r = 10;
            break;
        case 1:
            r = 20;
            break;
        case 2:
            r = 30;
            break;
    }
    return r;
}

int main(void) {
    return classify(0) + classify(1) + classify(2) + classify(9);
    /* 10 + 20 + 30 + 0 = 60 */
}
