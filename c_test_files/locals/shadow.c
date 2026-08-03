/* An inner-scope declaration shadows the outer one and dies at the `}`. */
int main(void) {
    int x = 1;
    {
        int x = 2;
        x = x + 10;
        if (x != 12) {
            return 100;
        }
    }
    return x; /* the outer x: 1 */
}
