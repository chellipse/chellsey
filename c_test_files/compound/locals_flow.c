/* Locals, assignment, and scoping woven through the expression tower and
   branches. Grows as features land. */
int main(void) {
    int acc = 0;
    int mask = 0xF0 >> 4;   /* 15 */
    acc = acc + (mask & 9); /* 9 */
    if ((acc == 9) & (mask > 10)) {
        int bonus = acc << 2; /* 36 */
        acc = acc + bonus;    /* 45 */
    } else {
        acc = acc - 1;
    }
    {
        int acc2 = acc * 2;
        acc = acc2 - acc; /* still 45 */
    }
    return acc + (mask != 15); /* 45 */
}
