/* `continue` in a while jumps straight to the condition re-test. */
int main(void) {
    int i = 0;
    int hits = 0;
    while (i < 10) {
        i = i + 1;
        if (i % 3 != 0) {
            continue;
        }
        hits = hits + 1;
    }
    return hits; /* i = 3, 6, 9 -> 3 */
}
