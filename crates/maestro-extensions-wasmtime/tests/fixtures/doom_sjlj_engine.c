/* SPDX-License-Identifier: MIT
 * A controlled fatal engine error returns through the component interface.
 */
#include "engine_errors.h"

extern void I_Error(char *format, ...);

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
    ENGINE_BEGIN(err);
    engine_step(milliseconds);
    ENGINE_END();
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
