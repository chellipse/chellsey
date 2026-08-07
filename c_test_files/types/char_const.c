/* Character constants have type int (6.4.4.4p10); a numeric escape's value
   is taken through `char` (signed here), so '\xff' is -1, not 255. */
int main(void) {
    if ('A' != 65) {
        return 1;
    }
    if ('\n' != 10 || '\t' != 9 || '\0' != 0) {
        return 2;
    }
    if ('\'' != 39 || '\\' != 92 || '\"' != 34) {
        return 3;
    }
    if ('\x41' != 'A' || '\101' != 'A') { /* hex and octal escapes */
        return 4;
    }
    if ('\xff' != -1) { /* through signed char */
        return 5;
    }
    if ('\377' != -1) { /* octal, same rule */
        return 6;
    }
    if ('a' + 1 != 'b') { /* plain int arithmetic */
        return 7;
    }
    char c = 'x'; /* and back into char storage */
    if (c != 120) {
        return 8;
    }
    unsigned char uc = '\xff'; /* int -1 converted to unsigned char: 255 */
    if (uc != 255) {
        return 9;
    }
    switch (c) { /* usable as a case label (integer constant expression) */
    case 'x':
        return 70;
    default:
        return 10;
    }
}
