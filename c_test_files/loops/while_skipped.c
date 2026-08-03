/* A condition false on entry: the body must not run even once. */
int main(void) {
    int x = 3;
    while (x < 3) {
        x = 100;
    }
    return x;
}
