/* `&&`/`||` yield exactly 0 or 1, whatever the operand values (6.5.13/14). */
int main(void) {
    return (1 && 1) * 32 + (1 && 0) * 16 + (0 || 1) * 8 + (0 || 0) * 4
        + (5 && -3) * 2 /* normalized: 1, not -3 */
        + (0 && 9);
    /* 32 + 0 + 8 + 0 + 2 + 0 = 42 */
}
