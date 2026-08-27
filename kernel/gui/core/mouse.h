#ifndef MOUSE_H
#define MOUSE_H

#include <stdint.h>

void mouse_init(void);

extern int32_t mouse_x, mouse_y;
extern uint8_t mouse_buttons;   /* bit0=左 bit1=右 bit2=中 */

#endif
