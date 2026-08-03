/* A multi-parameter definition compiles, encodes its prologue, and links —
   it can't be called until calls land, but it must not break the build. */
int add3(int a, int b, int c) {
    return a + b * c;
}

int main(void) {
    return 7;
}
