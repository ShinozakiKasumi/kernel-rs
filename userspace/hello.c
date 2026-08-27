#include "ulib.h"

void _start(void) {
    puts("[user] hello from ring3\n");
    puts("[user] write() works, exiting with code 7\n");
    sys_exit(7);
}
