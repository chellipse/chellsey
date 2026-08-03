/* Even with a condition false on entry, a do-while runs its body once — the
   property that distinguishes it from `while`. */
int main(void) {
    int count = 0;
    int i = 100;
    do {
        count++;
    } while (i < 5);  /* 100 < 5 is false, but the body already ran */
    return count;     /* 1 */
}
