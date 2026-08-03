/* A real incoming parameter: the runner passes no arguments, so argc is 1
   and the value flows in through rdi. */
int main(int argc) {
    return argc * 10 + 3; /* 13 */
}
