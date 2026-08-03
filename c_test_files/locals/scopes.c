/* Sibling scopes reuse a name; each gets its own storage. */
int main(void) {
    int r = 0;
    {
        int a = 5;
        r = r + a;
    }
    {
        int a = 7;
        r = r + a;
    }
    return r; /* 12 */
}
