/*
 * The hooks: routines of the original the probe steps into, to log what the
 * game does — every random draw, the screens the logic asks for, the dialogs,
 * the states of the game cycle — and later to steer it.
 *
 * An inline hook writes a jump to the probe over a routine's first whole
 * instructions and moves those to a trampoline that jumps back behind them,
 * so the probe still calls the original. Each hook names the bytes it moves;
 * if the routine starts differently, it is left alone. The moved bytes must
 * hold no relative jump or call, and no jump in the routine may lead into
 * them (tools/probe/prologues.py checks both). rand() and srand()
 * are hooked in the import tables of the modules that call them.
 *
 * Draws go to a trace file of 16-byte records (see struct draw), events to
 * the log as "event NAME key=value ...", each with the number of draws so far.
 */
#include <windows.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>

#include "probe.h"

/* The game's day, counted from the start of the first season. */
#define DAY (*(volatile short *)0xb5cbd4)

enum source { MAIN, SECOND, THIRD, RAND, RAND_SIM, SOURCES };
static const char *const source_names[SOURCES] = {"main", "second", "third", "rand",
						  "rand-activesim"};

/* One draw in the trace. */
struct draw {
	uint32_t caller;   /* the return address into the code that drew */
	uint32_t argument; /* the range for the generators, 0 for rand() */
	int32_t result;
	uint8_t source; /* enum source */
	uint8_t thread; /* the threads in the order they first drew, from 0 */
	uint16_t spare;
};

static volatile LONG counts[SOURCES];
static CRITICAL_SECTION trace_lock;
static FILE *trace;
static DWORD threads[255];
static int thread_count;
static volatile LONG crt_seed_fixed;
static volatile unsigned crt_seed;

static LONG total_draws(void)
{
	LONG total = 0;
	for (int i = 0; i < SOURCES; i++)
		total += counts[i];
	return total;
}

#define event(format, ...) note("event " format " draws=%ld", __VA_ARGS__, total_draws())

static void record(enum source source, void *caller, unsigned argument, int result)
{
	InterlockedIncrement(&counts[source]);
	if (!trace)
		return;
	EnterCriticalSection(&trace_lock);
	if (trace) {
		DWORD id = GetCurrentThreadId();
		int thread = 0;
		while (thread < thread_count && threads[thread] != id)
			thread++;
		if (thread == thread_count && thread_count < (int)(sizeof threads / sizeof *threads))
			threads[thread_count++] = id;
		struct draw draw = {(uint32_t)(uintptr_t)caller, argument, result, source,
				    (uint8_t)thread, 0};
		fwrite(&draw, sizeof draw, 1, trace);
	}
	LeaveCriticalSection(&trace_lock);
}

int trace_draws(const char *path)
{
	EnterCriticalSection(&trace_lock);
	if (trace)
		fclose(trace);
	trace = NULL;
	thread_count = 0;
	if (path) {
		trace = fopen(path, "wb");
		if (trace)
			setvbuf(trace, NULL, _IOFBF, 1 << 20);
	}
	LeaveCriticalSection(&trace_lock);
	return path && !trace ? -1 : 0;
}

void fix_crt_seed(int fixed, unsigned seed)
{
	crt_seed = seed;
	InterlockedExchange(&crt_seed_fixed, fixed);
}

void report_counts(char *reply, size_t size)
{
	size_t used = snprintf(reply, size, "ok");
	for (int i = 0; i < SOURCES && used < size; i++)
		used += snprintf(reply + used, size - used, " %s %ld", source_names[i], counts[i]);
}

/* The detours, and where each finds the original. */

typedef long(__cdecl *below_fn)(unsigned);
static below_fn below_original[3];

static long below(enum source source, unsigned range, void *caller)
{
	long value = below_original[source](range);
	record(source, caller, range, value);
	return value;
}

/* FUN_0078a810, FUN_0078a85c, FUN_0078a8a8: a draw below range from the
 * generators at 0x89dcc0, 0x8ad2b8 and 0x8ad2c0. */
static long __cdecl below_main(unsigned range)
{
	return below(MAIN, range, __builtin_return_address(0));
}

static long __cdecl below_second(unsigned range)
{
	return below(SECOND, range, __builtin_return_address(0));
}

static long __cdecl below_third(unsigned range)
{
	return below(THIRD, range, __builtin_return_address(0));
}

typedef int(__cdecl *rand_fn)(void);
typedef void(__cdecl *srand_fn)(unsigned);
static rand_fn rand_original[2];
static srand_fn srand_original[2];

static int __cdecl rand_game(void)
{
	int value = rand_original[0]();
	record(RAND, __builtin_return_address(0), 0, value);
	return value;
}

static int __cdecl rand_sim(void)
{
	int value = rand_original[1]();
	record(RAND_SIM, __builtin_return_address(0), 0, value);
	return value;
}

static void seed_crt(int module, unsigned seed, void *caller)
{
	unsigned used = crt_seed_fixed ? crt_seed : seed;
	event("srand module=%d seed=%u used=%u caller=%p", module, seed, used, caller);
	srand_original[module](used);
}

static void __cdecl srand_game(unsigned seed)
{
	seed_crt(0, seed, __builtin_return_address(0));
}

static void __cdecl srand_sim(unsigned seed)
{
	seed_crt(1, seed, __builtin_return_address(0));
}

/* FUN_00786b61(request): the logic thread waits until the screen it asked for
 * answers; the request is step, result, done (FLOW.md), and a flag at
 * 0x1c6aaec ends the thread instead. The original's loop jumps back into the
 * bytes the hook moves, so the detour waits itself and calls the original
 * only once it would not loop. */
#define QUITTING (*(volatile short *)0x1c6aaec)
typedef void(__cdecl *wait_fn)(short *);
static wait_fn wait_original;

static void __cdecl wait_for_screen(short *request)
{
	volatile short *done = &request[2];
	event("request step=%d day=%d", request[0], DAY);
	while (!*done && !QUITTING)
		Sleep(100);
	event("answer step=%d result=%d", request[0], request[1]);
	wait_original(request);
}

/* FUN_007a0bfd: a modal dialog's loop; the vtable tells the class. */
typedef int(__thiscall *modal_fn)(void *);
static modal_fn modal_original;

static int __thiscall modal(void *dialog)
{
	unsigned class = *(unsigned *)dialog;
	event("dialog class=%#x this=%p", class, dialog);
	int result = modal_original(dialog);
	event("dialog-end class=%#x result=%d", class, result);
	return result;
}

/* FUN_006da3a9: one round of the game cycle. */
typedef void(__cdecl *cycle_fn)(void);
static cycle_fn cycle_original, autosave_original;

static void __cdecl cycle(void)
{
	event("cycle day=%d", DAY);
	cycle_original();
}

/* FUN_006c03d7(state): the game cycle announces its state. */
typedef void(__cdecl *state_fn)(int);
static state_fn state_original;

static void __cdecl state(int value)
{
	event("state %d day=%d", (short)value, DAY);
	state_original(value);
}

/* FUN_006ecfeb: the weekly autosave. */
static void __cdecl autosave(void)
{
	event("autosave day=%d", DAY);
	autosave_original();
}

/* The inline hooks. */

struct hook {
	const char *name;
	uintptr_t address;
	/* The first whole instructions, moved to the trampoline: at least 5. */
	const char *prologue;
	size_t length;
	void *detour;
	void **original;
	const char *status;
};

#define HOOK(name, address, prologue, detour, original)                                        \
	{name, address, prologue, sizeof prologue - 1, (void *)detour, (void **)&original, "off"}

static struct hook hooks[] = {
	HOOK("below-main", 0x78a810, "\x55\x8b\xec\x83\xec\x10", below_main, below_original[MAIN]),
	HOOK("below-second", 0x78a85c, "\x55\x8b\xec\x83\xec\x10", below_second,
	     below_original[SECOND]),
	HOOK("below-third", 0x78a8a8, "\x55\x8b\xec\x83\xec\x10", below_third,
	     below_original[THIRD]),
	/* Its loop jumps back in: the detour waits itself. */
	HOOK("wait-for-screen", 0x786b61, "\x55\x8b\xec\x8b\x45\x08", wait_for_screen,
	     wait_original),
	HOOK("modal", 0x7a0bfd, "\x55\x8b\xec\x83\xec\x08", modal, modal_original),
	HOOK("cycle", 0x6da3a9, "\x55\x8b\xec\x83\xec\x24", cycle, cycle_original),
	HOOK("state", 0x6c03d7, "\x55\x8b\xec\x6a\x00", state, state_original),
	HOOK("autosave", 0x6ecfeb, "\x55\x8b\xec\x6a\xff", autosave, autosave_original),
};

static unsigned char *trampolines;

static void install(struct hook *hook)
{
	unsigned char *target = (unsigned char *)hook->address;
	if (memcmp(target, hook->prologue, hook->length)) {
		hook->status = "differs";
		note("hook %s: %#lx does not start as expected", hook->name,
		     (unsigned long)hook->address);
		return;
	}
	unsigned char *trampoline = trampolines;
	trampolines += 16;
	memcpy(trampoline, target, hook->length);
	trampoline[hook->length] = 0xe9;
	int32_t back = (int32_t)(target + hook->length - (trampoline + hook->length + 5));
	memcpy(trampoline + hook->length + 1, &back, 4);

	DWORD protection;
	VirtualProtect(target, hook->length, PAGE_EXECUTE_READWRITE, &protection);
	int32_t jump = (int32_t)((unsigned char *)hook->detour - (target + 5));
	target[0] = 0xe9;
	memcpy(target + 1, &jump, 4);
	memset(target + 5, 0x90, hook->length - 5);
	VirtualProtect(target, hook->length, protection, &protection);
	FlushInstructionCache(GetCurrentProcess(), target, hook->length);
	*hook->original = trampoline;
	hook->status = "on";
}

/* The import hooks: rand and srand of MSVCRT in the game and its match DLL. */

struct import {
	const char *name;
	const char *module, *library, *function;
	void *detour;
	void **original;
	const char *status;
};

static struct import imports[] = {
	{"rand", NULL, "msvcrt.dll", "rand", (void *)rand_game, (void **)&rand_original[0], "off"},
	{"srand", NULL, "msvcrt.dll", "srand", (void *)srand_game, (void **)&srand_original[0],
	 "off"},
	{"rand-activesim", "activesim.dll", "msvcrt.dll", "rand", (void *)rand_sim,
	 (void **)&rand_original[1], "off"},
	{"srand-activesim", "activesim.dll", "msvcrt.dll", "srand", (void *)srand_sim,
	 (void **)&srand_original[1], "off"},
};

static void patch_import(struct import *import)
{
	unsigned char *base = (unsigned char *)GetModuleHandleA(import->module);
	if (!base) {
		import->status = "no-module";
		return;
	}
	IMAGE_NT_HEADERS *nt = (IMAGE_NT_HEADERS *)(base + ((IMAGE_DOS_HEADER *)base)->e_lfanew);
	IMAGE_DATA_DIRECTORY *directory =
		&nt->OptionalHeader.DataDirectory[IMAGE_DIRECTORY_ENTRY_IMPORT];
	for (IMAGE_IMPORT_DESCRIPTOR *library =
		     (IMAGE_IMPORT_DESCRIPTOR *)(base + directory->VirtualAddress);
	     library->Name; library++) {
		if (_stricmp((char *)base + library->Name, import->library))
			continue;
		IMAGE_THUNK_DATA *names =
			(IMAGE_THUNK_DATA *)(base + library->OriginalFirstThunk);
		IMAGE_THUNK_DATA *slots = (IMAGE_THUNK_DATA *)(base + library->FirstThunk);
		for (; names->u1.AddressOfData; names++, slots++) {
			if (IMAGE_SNAP_BY_ORDINAL(names->u1.Ordinal))
				continue;
			IMAGE_IMPORT_BY_NAME *name =
				(IMAGE_IMPORT_BY_NAME *)(base + names->u1.AddressOfData);
			if (strcmp((char *)name->Name, import->function))
				continue;
			DWORD protection;
			VirtualProtect(&slots->u1.Function, sizeof slots->u1.Function,
				       PAGE_READWRITE, &protection);
			*import->original = (void *)slots->u1.Function;
			slots->u1.Function = (uintptr_t)import->detour;
			VirtualProtect(&slots->u1.Function, sizeof slots->u1.Function, protection,
				       &protection);
			import->status = "on";
			note("import %s: %s!%s was %p", import->name, import->library, import->function,
			     *import->original);
			return;
		}
	}
	import->status = "no-import";
}

void install_hooks(void)
{
	InitializeCriticalSection(&trace_lock);
	trampolines = VirtualAlloc(NULL, 4096, MEM_COMMIT | MEM_RESERVE, PAGE_EXECUTE_READWRITE);
	for (size_t i = 0; i < sizeof hooks / sizeof *hooks; i++)
		install(&hooks[i]);
	for (size_t i = 0; i < sizeof imports / sizeof *imports; i++)
		patch_import(&imports[i]);
	char status[512];
	report_hooks(status, sizeof status);
	note("hooks: %s", status + 3);
}

void report_hooks(char *reply, size_t size)
{
	size_t used = snprintf(reply, size, "ok");
	for (size_t i = 0; i < sizeof hooks / sizeof *hooks && used < size; i++)
		used += snprintf(reply + used, size - used, " %s:%s", hooks[i].name, hooks[i].status);
	for (size_t i = 0; i < sizeof imports / sizeof *imports && used < size; i++)
		used += snprintf(reply + used, size - used, " %s:%s", imports[i].name,
				 imports[i].status);
}
