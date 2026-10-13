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

#define event(format, ...)                                                                    \
	note("event " format " draws=%ld thread=%lu", __VA_ARGS__, total_draws(),               \
	     GetCurrentThreadId())

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

/* Dumps: the original's own savegame writer, FUN_006ed31e(path, 0, 0, 1) as
 * the autosave (FUN_006ecfeb) calls it, run on the logic thread at a safe
 * point — when it sleeps to wait for the player (its dozens of wait loops
 * poll a flag with Sleep(2) to Sleep(100); spin locks use Sleep(0)), or when the
 * game cycle announces a state; daily dumps at the last. Modal dialogs run on
 * the main thread meanwhile.
 * The path is an MFC CString, made and freed by the game's own thunks to
 * MFC42. */
typedef void(__cdecl *write_fn)(void *path, int, int, int);
typedef void *(__thiscall *cstring_make_fn)(void *self, const char *text);
typedef void(__thiscall *cstring_free_fn)(void *self);
#define WRITE_SAVE ((write_fn)0x6ed31e)
#define CSTRING_MAKE ((cstring_make_fn)0x7e09e0)
#define CSTRING_FREE ((cstring_free_fn)0x7e09e6)

static CRITICAL_SECTION dump_lock, dump_turn; /* the second: one dump at a time */
static volatile DWORD logic_thread; /* the thread of the game's logic */
static char dump_path[MAX_PATH]; /* asked for, empty when none */
static HANDLE dump_done;
static char dump_dir[MAX_PATH]; /* a dump on every new day into it */
static int dumped_day = -1;

static void write_save(const char *path)
{
	void *text = NULL;
	CSTRING_MAKE(&text, path);
	event("dump %s day=%d", path, DAY);
	WRITE_SAVE(&text, 0, 0, 1);
	CSTRING_FREE(&text);
}

/* On the logic thread, at a safe point; daily dumps only from the game
 * cycle, when there is a game. */
static void safe_point(BOOL in_cycle)
{
	char path[MAX_PATH] = "";
	EnterCriticalSection(&dump_lock);
	if (dump_path[0]) {
		strcpy(path, dump_path);
		dump_path[0] = 0;
	}
	char dir[MAX_PATH];
	strcpy(dir, dump_dir);
	LeaveCriticalSection(&dump_lock);
	if (path[0]) {
		write_save(path);
		SetEvent(dump_done);
	}
	if (in_cycle && dir[0] && DAY != dumped_day) {
		char daily[MAX_PATH + 16];
		dumped_day = DAY;
		snprintf(daily, sizeof daily, "%s\\day-%03d.an3", dir, DAY);
		write_save(daily);
	}
}

int dump(const char *path, DWORD timeout)
{
	if (strlen(path) >= MAX_PATH)
		return -1;
	EnterCriticalSection(&dump_turn);
	int result = -1;
	EnterCriticalSection(&dump_lock);
	ResetEvent(dump_done);
	strcpy(dump_path, path);
	LeaveCriticalSection(&dump_lock);
	if (WaitForSingleObject(dump_done, timeout) == WAIT_OBJECT_0) {
		result = 0;
	} else {
		EnterCriticalSection(&dump_lock);
		dump_path[0] = 0;
		LeaveCriticalSection(&dump_lock);
	}
	LeaveCriticalSection(&dump_turn);
	return result;
}

void report_logic_thread(char *reply, size_t size)
{
	DWORD id = logic_thread;
	HANDLE thread = id ? OpenThread(THREAD_SUSPEND_RESUME | THREAD_GET_CONTEXT, FALSE, id) : NULL;
	if (!thread) {
		snprintf(reply, size, "error no logic thread yet");
		return;
	}
	CONTEXT context = {.ContextFlags = CONTEXT_CONTROL | CONTEXT_INTEGER};
	SuspendThread(thread);
	BOOL got = GetThreadContext(thread, &context);
	size_t used = snprintf(reply, size, "ok thread %lu eip %#lx", id, got ? context.Eip : 0);
	/* The frames by their saved EBP, while they lie on readable stack. */
	DWORD frame = got ? context.Ebp : 0;
	for (int depth = 0; depth < 24 && frame && used < size; depth++) {
		MEMORY_BASIC_INFORMATION info;
		if (!VirtualQuery((void *)frame, &info, sizeof info) || info.State != MEM_COMMIT)
			break;
		DWORD *pair = (DWORD *)frame;
		used += snprintf(reply + used, size - used, " %#lx", pair[1]);
		if (pair[0] <= frame)
			break;
		frame = pair[0];
	}
	ResumeThread(thread);
	CloseHandle(thread);
}

/* A write watchpoint without debug registers (Wine does not pass them on):
 * the page holding the address is made read-only; a write to it faults, is
 * logged if it hits the two watched bytes, and goes through with the page
 * writable for one single step, after which the page is read-only again. */
static volatile DWORD watched;
static volatile LONG rearm;

static void log_frames(const CONTEXT *context, char *out, size_t size)
{
	size_t used = 0;
	DWORD frame = context->Ebp;
	out[0] = 0;
	for (int depth = 0; depth < 12 && frame && used < size; depth++) {
		MEMORY_BASIC_INFORMATION info;
		if (!VirtualQuery((void *)frame, &info, sizeof info) || info.State != MEM_COMMIT)
			break;
		DWORD *pair = (DWORD *)frame;
		used += snprintf(out + used, size - used, " %#lx", pair[1]);
		if (pair[0] <= frame)
			break;
		frame = pair[0];
	}
}

static void protect(DWORD protection)
{
	DWORD old;
	VirtualProtect((void *)(watched & ~0xffful), 0x1000, protection, &old);
}

static LONG CALLBACK on_watch(EXCEPTION_POINTERS *info)
{
	EXCEPTION_RECORD *record = info->ExceptionRecord;
	if (record->ExceptionCode == EXCEPTION_ACCESS_VIOLATION && watched &&
	    record->ExceptionInformation[0] == 1 &&
	    (record->ExceptionInformation[1] & ~0xffful) == (watched & ~0xffful)) {
		ULONG_PTR at = record->ExceptionInformation[1];
		if (at + 4 > watched && at < watched + 2) {
			char frames[256];
			log_frames(info->ContextRecord, frames, sizeof frames);
			event("watch %#lx = %d before the write at %p, frames%s", watched,
			      *(volatile short *)watched, record->ExceptionAddress, frames);
		}
		protect(PAGE_READWRITE);
		InterlockedExchange(&rearm, 1);
		info->ContextRecord->EFlags |= 0x100; /* trap after the write */
		return EXCEPTION_CONTINUE_EXECUTION;
	}
	if (record->ExceptionCode == EXCEPTION_SINGLE_STEP && InterlockedExchange(&rearm, 0)) {
		protect(PAGE_READONLY);
		info->ContextRecord->EFlags &= ~0x100ul;
		return EXCEPTION_CONTINUE_EXECUTION;
	}
	return EXCEPTION_CONTINUE_SEARCH;
}

int watch(DWORD address)
{
	static PVOID handler;
	if (!handler)
		handler = AddVectoredExceptionHandler(1, on_watch);
	watched = address;
	protect(PAGE_READONLY);
	return 1;
}

void dump_daily(const char *dir)
{
	EnterCriticalSection(&dump_lock);
	snprintf(dump_dir, sizeof dump_dir, "%s", dir ? dir : "");
	dumped_day = -1;
	LeaveCriticalSection(&dump_lock);
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
	logic_thread = GetCurrentThreadId();
	event("request step=%d day=%d", request[0], DAY);
	while (!*done && !QUITTING) {
		safe_point(FALSE);
		Sleep(100);
	}
	event("answer step=%d result=%d", request[0], request[1]);
	wait_original(request);
}

/* FUN_007a0bfd: a modal dialog's loop; the vtable tells the class. */
static void *modals[16];
static volatile LONG modal_count;
typedef int(__thiscall *modal_fn)(void *);
static modal_fn modal_original;

static int __thiscall modal(void *dialog)
{
	unsigned class = *(unsigned *)dialog;
	LONG depth = InterlockedIncrement(&modal_count);
	if (depth <= (LONG)(sizeof modals / sizeof *modals))
		modals[depth - 1] = dialog;
	event("dialog class=%#x this=%p", class, dialog);
	int result = modal_original(dialog);
	event("dialog-end class=%#x result=%d", class, result);
	InterlockedDecrement(&modal_count);
	return result;
}

static BOOL is_modal(void *dialog)
{
	LONG depth = modal_count;
	for (LONG i = 0; i < depth && i < (LONG)(sizeof modals / sizeof *modals); i++) {
		if (modals[i] == dialog)
			return TRUE;
	}
	return FALSE;
}

/* The dialogs the game shows, in the order shown: each a CMyDialog, its id
 * in the dialog file at +0xb4, flags at +0x44, of which 0x10 is set while it
 * is up, and the window it belongs to at +0x50. FUN_007a0a79 shows one,
 * FUN_0079f127 destroys it; those in DoModal are marked modal. */
#define DIALOG_ID(dialog) (*(int *)((char *)(dialog) + 0xb4))
#define DIALOG_FLAGS(dialog) (*(unsigned *)((char *)(dialog) + 0x44))
#define DIALOG_PARENT(dialog) (*(void **)((char *)(dialog) + 0x50))
static CRITICAL_SECTION dialogs_lock;
static void *dialogs[64];
static int dialog_count;
static volatile LONG dialogs_shown;

typedef int(__thiscall *show_fn)(void *);
typedef void(__thiscall *destroy_fn)(void *);
static show_fn show_original;
static destroy_fn destroy_original;

static void forget(void *dialog)
{
	for (int i = 0; i < dialog_count; i++) {
		if (dialogs[i] == dialog) {
			memmove(&dialogs[i], &dialogs[i + 1], (dialog_count - i - 1) * sizeof *dialogs);
			dialog_count--;
			return;
		}
	}
}

static int __thiscall show(void *dialog)
{
	EnterCriticalSection(&dialogs_lock);
	forget(dialog);
	if (dialog_count < (int)(sizeof dialogs / sizeof *dialogs))
		dialogs[dialog_count++] = dialog;
	InterlockedIncrement(&dialogs_shown);
	LeaveCriticalSection(&dialogs_lock);
	event("show dialog=%d this=%p", DIALOG_ID(dialog), dialog);
	return show_original(dialog);
}

static void __thiscall destroy(void *dialog)
{
	EnterCriticalSection(&dialogs_lock);
	forget(dialog);
	LeaveCriticalSection(&dialogs_lock);
	destroy_original(dialog);
}

void report_dialogs(char *reply, size_t size)
{
	EnterCriticalSection(&dialogs_lock);
	size_t used = snprintf(reply, size, "ok %ld", dialogs_shown);
	for (int i = 0; i < dialog_count && used < size; i++)
		used += snprintf(reply + used, size - used, " %d:%p:%#x:%p:%d", DIALOG_ID(dialogs[i]),
				 dialogs[i], DIALOG_FLAGS(dialogs[i]), DIALOG_PARENT(dialogs[i]),
				 is_modal(dialogs[i]));
	LeaveCriticalSection(&dialogs_lock);
}

/* FUN_006da3a9: the game cycle, which runs a whole season. */
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
	logic_thread = GetCurrentThreadId();
	event("state %d day=%d", (short)value, DAY);
	safe_point(TRUE);
	state_original(value);
}

/* Sleep, as the game calls it: a wait of the logic thread is a safe point. */
typedef void(WINAPI *sleep_fn)(DWORD);
static sleep_fn sleep_original;

static void WINAPI game_sleep(DWORD ms)
{
	if (ms > 0 && GetCurrentThreadId() == logic_thread)
		safe_point(FALSE);
	sleep_original(ms);
}

/* FUN_006ecfeb: the weekly autosave. */
static void __cdecl autosave(void)
{
	event("autosave day=%d", DAY);
	autosave_original();
}

/*
 * FUN_004316a2 picks the display mode from the list of modes it was given
 * (12 bytes each, the width first): with none 640 wide it reads far past the
 * list and the game crashes. Now and then, starting cold, Wine lists no
 * 640×480; Windows always has it, so it is added then.
 */
typedef void(__cdecl *modes_fn)(int *, int *);
static modes_fn modes_original;
/* The list's room: from 0x1c6bc48 to its count at 0x1c6bf1c. */
#define MODES_ROOM 60

static void __cdecl modes(int *count, int *list)
{
	int i = 0;
	while (i < *count && list[3 * i] != 640)
		i++;
	if (i == *count && *count < MODES_ROOM) {
		event("modes none-of=%d 640x480 added", *count);
		list[3 * i] = 640;
		list[3 * i + 1] = 480;
		list[3 * i + 2] = i ? list[2] : 16;
		++*count;
	}
	modes_original(count, list);
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
	HOOK("show", 0x7a0a79, "\x55\x8b\xec\x6a\xff", show, show_original),
	HOOK("destroy", 0x79f127, "\x55\x8b\xec\x6a\xff", destroy, destroy_original),
	HOOK("modes", 0x4316a2, "\x55\x8b\xec\x83\xec\x14", modes, modes_original),
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

/* The import hooks: rand and srand of MSVCRT in the game and its match DLL,
 * and the game's Sleep. */

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
	{"sleep", NULL, "kernel32.dll", "Sleep", (void *)game_sleep, (void **)&sleep_original,
	 "off"},
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
	InitializeCriticalSection(&dialogs_lock);
	InitializeCriticalSection(&dump_lock);
	InitializeCriticalSection(&dump_turn);
	dump_done = CreateEventA(NULL, TRUE, FALSE, NULL);
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
