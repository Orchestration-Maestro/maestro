/* SPDX-License-Identifier: MIT
 * A controlled fatal engine error returns through the component interface.
 */
#include "engine_errors.h"
#include <string.h>

extern void I_Error(char *format, ...);

static void engine_step(uint32_t milliseconds) {
    if (milliseconds == 28) {
        I_Error("forced engine failure: %d", 7);
    }
    if (milliseconds == 29) I_Error("invalid:\xff");
    if (milliseconds == 30) {
        char text[1025];
        memset(text, 'a', 1021);
        memcpy(text + 1021, "\xe2\x82\xac", 4);
        I_Error("%s", text);
    }
    if (milliseconds == 31) I_Error("valid:\xc3\xa9\xe2\x82\xac\xf0\x90\x80\x80\xf4\x8f\xbf\xbf");
    if (milliseconds == 32) I_Error("sequence:\xff\xc0\xaf\xe0\x80\xaf\xed\xa0\x80\xf0\x80\x80\xaf\xf4\x90\x80\x80\xe2\x82x");
    if (milliseconds == 33) I_Error("incomplete:\xe2\x82");
}

bool exports_maestro_doom_engine_init(game_list_u8_t *wad, game_string_t *root, game_string_t *err) {
    (void)err;
    game_list_u8_free(wad);
    game_string_free(root);
    return true;
}

bool exports_maestro_doom_engine_tick(uint32_t milliseconds, game_string_t *err) {
    ENGINE_BEGIN(err);
    engine_step(milliseconds);
    ENGINE_END();
    return true;
}

void exports_maestro_doom_engine_dimensions(game_tuple2_u32_u32_t *ret) {
    ret->f0 = 1;
    ret->f1 = 1;
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
