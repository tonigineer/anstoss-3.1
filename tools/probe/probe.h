/* What probe.c and hooks.c share. */
#ifndef PROBE_H
#define PROBE_H

#include <stddef.h>
#include <windows.h>

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
/* Has the logic thread write the game's state to path, as a savegame, at its
 * next safe point; 0 once written, -1 if it did not come within timeout ms. */
int dump(const char *path, DWORD timeout);
/* A dump on every new day into dir (day-NNN.an3), or none (dir NULL). */
void dump_daily(const char *dir);
/* "thread ID eip EIP RET ...": where the logic thread is, and the return
 * addresses of its frames. */
void report_logic_thread(char *reply, size_t size);
/* Logs every write to the two bytes at address (with page protection); 1. */
int watch(DWORD address);
/* Makes every later srand() use seed, or the game's own again (fixed 0). */
void fix_crt_seed(int fixed, unsigned seed);

#endif
