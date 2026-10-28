/* SPDX-License-Identifier: MIT
 * Engine console output is discarded; configuration and save output is retained.
 */
#include <stdarg.h>
#include <stddef.h>
#include <stdio.h>

int doom_printf(const char *format, ...) asm("__wrap_printf");
int doom_puts(const char *text) asm("__wrap_puts");
int doom_putchar(int value) asm("__wrap_putchar");
int doom_fprintf(FILE *file, const char *format, ...) asm("__wrap_fprintf");
int doom_vfprintf(FILE *file, const char *format, va_list args) asm("__wrap_vfprintf");
size_t doom_fwrite(const void *data, size_t size, size_t count, FILE *file) asm("__wrap_fwrite");
int file_vfprintf(FILE *file, const char *format, va_list args) asm("__real_vfprintf");
size_t file_fwrite(const void *data, size_t size, size_t count, FILE *file) asm("__real_fwrite");

int doom_printf(const char *format, ...) { (void)format; return 0; }
int doom_puts(const char *text) { (void)text; return 0; }
int doom_putchar(int value) { return value; }
int doom_vfprintf(FILE *file, const char *format, va_list args) {
    if (file == stdout || file == stderr) return 0;
    return file_vfprintf(file, format, args);
}
int doom_fprintf(FILE *file, const char *format, ...) {
    va_list args;
    va_start(args, format);
    int result = doom_vfprintf(file, format, args);
    va_end(args);
    return result;
}
size_t doom_fwrite(const void *data, size_t size, size_t count, FILE *file) {
    if (file == stdout || file == stderr) return size ? count : 0;
    return file_fwrite(data, size, count, file);
}
