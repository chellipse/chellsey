/* Nested if/else; inner else taken, then fall through past the outer if. */
int main(void) {
    if (2 > 1) {
        if (4 > 5) return 20;
        else return 10;
    }
    return 30;
}
