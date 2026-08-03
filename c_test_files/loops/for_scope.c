/* The for clause's declaration shadows the outer name and dies at loop end. */
int main(void) {
    int i = 100;
    int last = 0;
    for (int i = 0; i < 3; i = i + 1) {
        last = i;
    }
    return i + last; /* 100 + 2 = 102 */
}
