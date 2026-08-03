/* Both arms reachable across uses; the result is the arm's value, not a
   0/1 truth value: (5 ? 7 : 9) is 7. */
int main(void) {
    return (1 ? 4 : 9) * 10 + (0 ? 3 : 2) + (5 ? 7 : 9) - 7; /* 40+2+7-7 = 42 */
}
