/* SPDX-License-Identifier: MIT
 * Controlled private-root filesystem effects behind the engine interface.
 * This fixture contains no game code or game data.
 */
#include "game.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static char *root;
static uint8_t *wad;
static size_t wad_len;
static bool failed;

static bool save(const char *name, const void *data, size_t len) {
    size_t size = strlen(root) + strlen(name) + 2;
    char *path = malloc(size);
    if (!path) return false;
    snprintf(path, size, "%s/%s", root, name);
    FILE *file = fopen(path, "wb");
    free(path);
    if (!file) return false;
    bool ok = fwrite(data, 1, len, file) == len;
    if (fclose(file) != 0) ok = false;
    return ok;
}

bool exports_maestro_doom_engine_init(game_list_u8_t *data,
                                     game_string_t *directory,
                                     game_string_t *error) {
    root = malloc(directory->len + 1);
    wad = malloc(data->len ? data->len : 1);
    if (!root || !wad) {
        game_string_dup(error, "controlled engine allocation failed");
        return false;
    }
    memcpy(root, directory->ptr, directory->len);
    root[directory->len] = 0;
    if (data->len) memcpy(wad, data->ptr, data->len);
    wad_len = data->len;
    game_list_u8_free(data);
    game_string_free(directory);
    failed = !save("doom1.wad", wad, wad_len) ||
             !save("config.cfg", "controlled config", 17) || wad_len == 0;
    if (failed) game_string_dup(error, "controlled engine file creation failed");
    return !failed;
}

bool exports_maestro_doom_engine_tick(uint32_t milliseconds,
                                     game_string_t *error) {
    (void)milliseconds;
    if (wad_len == 10 && memcmp(wad, "tick-error", 10) == 0) {
        game_string_dup(error, "controlled tick failure");
        return false;
    }
    return true;
}

void exports_maestro_doom_engine_key(bool pressed, uint8_t key) {
    if (pressed && key == 0xa2) failed = !save("save.dat", wad, wad_len);
}

void exports_maestro_doom_engine_dimensions(game_tuple2_u32_u32_t *ret) {
    bool small = wad_len == 11 && memcmp(wad, "small-frame", 11) == 0;
    ret->f0 = small ? 320 : 640;
    ret->f1 = small ? 200 : 400;
}

void exports_maestro_doom_engine_frame(game_list_u8_t *frame) {
    size_t width = wad_len == 11 && memcmp(wad, "small-frame", 11) == 0 ? 320 : 640;
    size_t height = width == 320 ? 200 : 400;
    bool malformed = wad_len == 9 && memcmp(wad, "bad-frame", 9) == 0;
    frame->len = failed || malformed ? 0 : width * height * 4;
    frame->ptr = calloc(frame->len, 1);
    if (!frame->ptr) frame->len = 0;
    for (size_t index = 0; index < frame->len / 4; ++index) {
        frame->ptr[index * 4] = index % width;
        frame->ptr[index * 4 + 1] = index / width;
        frame->ptr[index * 4 + 3] = 255;
    }
}
