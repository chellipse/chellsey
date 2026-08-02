/* Unary ~ binds tighter than binary &: (~5) & 255 = -6 & 255 = 250. */
int main(void) { return ~5 & 255; }
