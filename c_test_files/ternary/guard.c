/* A division guarded by the condition: the untaken arm would divide by
   zero if it ever evaluated. */
int main(void) {
    int acc = 0;
    for (int b = -2; b <= 2; b = b + 1) {
        acc = acc + (b == 0 ? 0 : 60 / b);
    }
    /* -30 + -60 + 0 + 60 + 30 = 0 */
    return acc + 42;
}
