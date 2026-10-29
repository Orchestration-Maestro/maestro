/* SPDX-License-Identifier: MIT
 * Headless platform and component exports for a separately built engine.
 */
#include "game.h"
#include "engine_errors.h"
#include "doomgeneric.h"
#include <errno.h>
#include <stdint.h>
#include <string.h>
#include <sys/stat.h>
#include <unistd.h>

static uint32_t clock_ms;
static bool initialized;
static struct { int pressed; unsigned char key; } keys[256];
static unsigned read_key;
static unsigned write_key;
static char *engine_args[] = { "doom", "-iwad", "doom1.wad", "-config", "default.cfg", "-savedir", "saves", "-nosound", "-nomusic" };

void DG_Init(void) {}
void DG_DrawFrame(void) {}
void DG_SetWindowTitle(const char *title) { (void)title; }
void DG_SleepMs(uint32_t milliseconds) { clock_ms += milliseconds; }
uint32_t DG_GetTicksMs(void) { return clock_ms; }
int DG_GetKey(int *pressed, unsigned char *key) {
    if (read_key == write_key) return 0;
    *pressed = keys[read_key].pressed;
    *key = keys[read_key].key;
    read_key = (read_key + 1) % 256;
    return 1;
}

static void prepare(game_list_u8_t *wad, game_string_t *root) {
    char *path = malloc(root->len + 1);
    if (!path) doom_error("Cannot allocate the engine directory name");
    memcpy(path, root->ptr, root->len);
    path[root->len] = 0;
    if (chdir(path) != 0) doom_error("Cannot enter the engine directory: %s", strerror(errno));
    if (setenv("HOME", path, 1) != 0 || setenv("XDG_CONFIG_HOME", path, 1) != 0 ||
        setenv("XDG_DATA_HOME", path, 1) != 0 || setenv("TMPDIR", path, 1) != 0)
        doom_error("Cannot isolate the engine environment: %s", strerror(errno));
    free(path);
    if (mkdir("saves", 0700) != 0 && errno != EEXIST)
        doom_error("Cannot create the save directory: %s", strerror(errno));
    FILE *file = fopen("doom1.wad", "wb");
    if (!file) doom_error("Cannot create the private WAD: %s", strerror(errno));
    bool written = fwrite(wad->ptr, 1, wad->len, file) == wad->len;
    if (fclose(file) != 0) written = false;
    if (!written) doom_error("Cannot write the private WAD: %s", strerror(errno));
}

bool exports_maestro_doom_engine_init(game_list_u8_t *wad, game_string_t *root, game_string_t *error) {
    ENGINE_BEGIN(error);
    if (initialized) doom_error("Restart requires a fresh engine component");
    initialized = true;
    prepare(wad, root);
    game_list_u8_free(wad);
    game_string_free(root);
    doomgeneric_Create(sizeof(engine_args) / sizeof(engine_args[0]), engine_args);
    ENGINE_END();
    return true;
}

bool exports_maestro_doom_engine_tick(uint32_t milliseconds, game_string_t *error) {
    ENGINE_BEGIN(error);
    if (!initialized) doom_error("The engine needs a fresh component");
    if (milliseconds > clock_ms) clock_ms = milliseconds;
    doomgeneric_Tick();
    ENGINE_END();
    return true;
}

void exports_maestro_doom_engine_key(bool pressed, uint8_t key) {
    if (!initialized) return;
    unsigned next = (write_key + 1) % 256;
    if (next == read_key) return;
    keys[write_key].pressed = pressed;
    keys[write_key].key = key;
    write_key = next;
}

void exports_maestro_doom_engine_dimensions(game_tuple2_u32_u32_t *ret) {
    ret->f0 = DOOMGENERIC_RESX;
    ret->f1 = DOOMGENERIC_RESY;
}

void exports_maestro_doom_engine_frame(game_list_u8_t *frame) {
    frame->ptr = NULL;
    frame->len = 0;
    if (!initialized || !DG_ScreenBuffer) return;
    frame->ptr = malloc((size_t)DOOMGENERIC_RESX * DOOMGENERIC_RESY * 4);
    if (!frame->ptr) return;
    frame->len = (size_t)DOOMGENERIC_RESX * DOOMGENERIC_RESY * 4;
    for (size_t index = 0; index < (size_t)DOOMGENERIC_RESX * DOOMGENERIC_RESY; ++index) {
        uint32_t pixel = DG_ScreenBuffer[index];
        frame->ptr[index * 4] = (pixel >> 16) & 255;
        frame->ptr[index * 4 + 1] = (pixel >> 8) & 255;
        frame->ptr[index * 4 + 2] = pixel & 255;
        frame->ptr[index * 4 + 3] = 255;
    }
}
