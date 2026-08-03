/* `break` exits only the innermost loop. */
int main(void) {
    int count = 0;
    for (int i = 0; i < 5; i = i + 1) {
        for (int j = 0; j < 5; j = j + 1) {
            if (j > i) {
                break;
            }
            count = count + 1;
        }
    }
    return count; /* 1+2+3+4+5 = 15 */
}
