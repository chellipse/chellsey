/* A switch over a long discriminant: case values beyond 32 bits dispatch
   exactly, and near-miss low-bits values must not match. */
int classify(long v) {
    switch (v) {
        case 4294967296L: /* 2^32 */
            return 10;
        case 42:
            return 20;
        default:
            return 30;
    }
}

int main(void) {
    /* 2^32 and 0 share low 32 bits; only the former is case 2^32 */
    return classify(4294967296L) + classify(42) + classify(0); /* 10+20+30 */
}
