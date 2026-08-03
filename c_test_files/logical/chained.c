/* Chains and mixed precedence: `&&` binds tighter than `||`. */
int main(void) {
    int t = 1 && 1 && 1; /* 1 */
    int f = 1 && 1 && 0; /* 0 */
    int m = 0 && 0 || 1; /* (0 && 0) || 1 = 1 */
    int n = 1 || 0 && 0; /* 1 || (0 && 0) = 1 */
    return t * 8 + f * 4 + m * 2 + n; /* 8 + 0 + 2 + 1 = 11 */
}
