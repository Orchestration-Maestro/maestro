/* SPDX-License-Identifier: MIT
 * A controlled fatal engine error returns through the component interface.
 */
#include "game.h"
#include <setjmp.h>
#include <stdarg.h>
#include <stdio.h>
#include <stdlib.h>

static jmp_buf fatal;
static char message[512];

extern void I_Error(char *format, ...);

void doom_error(char *format, ...) asm("__wrap_I_Error");

void doom_error(char *format, ...) {
    va_list args;
    va_start(args, format);
    vsnprintf(message, sizeof(message), format, args);
    va_end(args);
    longjmp(fatal, 1);
}

static void engine_step(uint32_t milliseconds) {
    if (milliseconds == 28) {
        I_Error("forced engine failure: %d", 7);
    }
}

bool exports_maestro_doom_engine_init(game_list_u8_t *wad, game_string_t *root, game_string_t *err) {
    (void)err;
    game_list_u8_free(wad);
    game_string_free(root);
    return true;
}

bool exports_maestro_doom_engine_tick(uint32_t milliseconds, game_string_t *err) {
    if (setjmp(fatal)) {
        game_string_dup(err, message);
        return false;
    }
    engine_step(milliseconds);
    return true;
}

void exports_maestro_doom_engine_frame(game_list_u8_t *ret) {
    ret->ptr = malloc(4);
    ret->len = 4;
    ret->ptr[0] = 1;
    ret->ptr[1] = 2;
    ret->ptr[2] = 3;
    ret->ptr[3] = 255;
}

void exports_maestro_doom_engine_key(bool pressed, uint8_t key) {
    (void)pressed;
    (void)key;
}
