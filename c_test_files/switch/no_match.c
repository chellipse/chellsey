/* When nothing matches and there is no default, the switch does nothing. The
   discriminant here is a constant, exercising a constant-valued switch. */
int main(void) {
    int r = 7;
    switch (100) {
        case 0:
            r = 0;
            break;
        case 1:
            r = 1;
            break;
    }
    return r; /* unchanged: 7 */
}
