/* SPDX-License-Identifier: MIT
 * The calling export owns the C stack boundary for engine failures.
 */
#ifndef MAESTRO_DOOM_ENGINE_ERRORS_H
#define MAESTRO_DOOM_ENGINE_ERRORS_H
#include "game.h"
#include <setjmp.h>
#include <stdarg.h>
#include <stdio.h>
#include <stdlib.h>

static jmp_buf engine_boundary;
static char engine_message[1024];
static bool engine_entered;

void doom_error(char *format, ...) asm("__wrap_I_Error");
void doom_exit(int status) asm("__wrap_exit");

void doom_error(char *format, ...) {
    va_list args;
    va_start(args, format);
    vsnprintf(engine_message, sizeof(engine_message), format, args);
    va_end(args);
    if (engine_entered) longjmp(engine_boundary, 1);
    abort();
}

void doom_exit(int status) {
    doom_error("DOOM exited with status %d", status);
}

static bool engine_failure(game_string_t *error) {
    engine_entered = false;
    game_string_dup(error, engine_message);
    return false;
}

#define ENGINE_BEGIN(error) \
    do { \
        engine_entered = true; \
        if (setjmp(engine_boundary)) return engine_failure(error); \
    } while (false)
#define ENGINE_END() do { engine_entered = false; } while (false)
#endif
