/* Comma threaded with a call inside a loop: bump the counter, then fold it in
   through a helper. */
int inc_by(int base, int amt) {
    return base + amt;
}

int main(void) {
    int acc = 0;
    int i = 0;
    while (i < 4) {
        acc = (i = i + 1, inc_by(acc, i)); /* i:1 acc=1; 2 -> 3; 3 -> 6; 4 -> 10 */
    }
    return acc; /* 10 */
}
