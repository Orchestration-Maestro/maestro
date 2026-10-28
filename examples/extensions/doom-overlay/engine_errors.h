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
#include <string.h>

static jmp_buf engine_boundary;
static char engine_message[1024];
static bool engine_entered;
static bool engine_truncated;

void doom_error(char *format, ...) asm("__wrap_I_Error");
void doom_exit(int status) asm("__wrap_exit");

void doom_error(char *format, ...) {
    va_list args;
    va_start(args, format);
    engine_message[0] = 0;
    int length = vsnprintf(engine_message, sizeof(engine_message), format, args);
    engine_message[sizeof(engine_message) - 1] = 0;
    engine_truncated = length >= (int)sizeof(engine_message);
    va_end(args);
    if (engine_entered) longjmp(engine_boundary, 1);
    abort();
}

void doom_exit(int status) {
    doom_error("DOOM exited with status %d", status);
}

static size_t utf8_width(unsigned char first) {
    if (first < 0x80) return 1;
    if (first >= 0xc2 && first <= 0xdf) return 2;
    if (first >= 0xe0 && first <= 0xef) return 3;
    if (first >= 0xf0 && first <= 0xf4) return 4;
    return 0;
}

static bool utf8_continuation(unsigned char first, unsigned char byte, size_t position) {
    if (byte < 0x80 || byte > 0xbf) return false;
    if (position != 1) return true;
    return (first != 0xe0 || byte >= 0xa0) && (first != 0xed || byte <= 0x9f) &&
           (first != 0xf0 || byte >= 0x90) && (first != 0xf4 || byte <= 0x8f);
}

static const char *engine_diagnostic(void) {
    /* Each invalid byte can expand to the three-byte replacement character. */
    static char result[sizeof(engine_message) * 3];
    const unsigned char *text = (const unsigned char *)engine_message;
    size_t length = strlen(engine_message), cursor = 0, output = 0;
    while (cursor < length) {
        size_t width = utf8_width(text[cursor]), prefix = 1;
        while (prefix < width && cursor + prefix < length &&
               utf8_continuation(text[cursor], text[cursor + prefix], prefix)) ++prefix;
        if (width && prefix == width) {
            memcpy(result + output, text + cursor, width);
            output += width;
        } else if (width && cursor + prefix == length && engine_truncated) {
            break;
        } else {
            memcpy(result + output, "\xef\xbf\xbd", 3);
            output += 3;
        }
        cursor += prefix;
    }
    result[output] = 0;
    return result;
}

static bool engine_failure(game_string_t *error) {
    engine_entered = false;
    game_string_dup(error, engine_diagnostic());
    return false;
}

#define ENGINE_BEGIN(error) \
    do { \
        engine_entered = true; \
        if (setjmp(engine_boundary)) return engine_failure(error); \
    } while (false)
#define ENGINE_END() do { engine_entered = false; } while (false)
#endif
