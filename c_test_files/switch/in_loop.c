/* `break` inside a switch leaves the switch (not the enclosing loop), while
   `continue` passes through the switch to the loop's step. */
int main(void) {
    int sum = 0;
    int hits = 0;
    for (int i = 0; i < 6; i++) {
        switch (i) {
            case 0:
            case 1:
                continue; /* jump to the for-step: i=0,1 contribute nothing */
            case 4:
                break;    /* leave the switch; the code after it still runs */
            default:
                sum += i;
        }
        hits++;           /* reached only when the switch didn't `continue` */
    }
    /* i=0,1: continue. i=2: sum+=2, hit. i=3: sum+=3, hit. i=4: break, hit.
       i=5: sum+=5, hit. -> sum=10, hits=4 */
    return sum * 10 + hits; /* 104 */
}
