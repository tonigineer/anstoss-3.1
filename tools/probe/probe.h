/* What probe.c and hooks.c share. */
#ifndef PROBE_H
#define PROBE_H

#include <stddef.h>

/* A line in the probe's log, with the time. */
void note(const char *format, ...) __attribute__((format(printf, 1, 2)));

/* Installs the hooks; at load, while the game's threads still wait. */
void install_hooks(void);
/* "name on|off|differs ..." for each hook. */
void report_hooks(char *reply, size_t size);
/* "SHOWN ID:THIS:FLAGS:PARENT:MODAL ...": how many dialogs were shown, and
 * those still there, in the order shown. */
void report_dialogs(char *reply, size_t size);
/* The draws so far, by source. */
void report_counts(char *reply, size_t size);
/* Starts writing every draw to path, or stops (path NULL); 0 on success. */
int trace_draws(const char *path);
/* Makes every later srand() use seed, or the game's own again (fixed 0). */
void fix_crt_seed(int fixed, unsigned seed);

#endif
