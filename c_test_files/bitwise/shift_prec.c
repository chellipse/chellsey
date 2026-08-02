/* `+` binds tighter than `<<`: (1 + 1) << 2 = 8. */
int main(void) { return 1 + 1 << 2; }
