/*
 * The probe inside the original: loaded by launch.exe before the game runs,
 * it keeps a log and answers commands, one line each, on a TCP socket on
 * localhost — port $A3PROBE_PORT, 47300 by default. From the start,
 * $A3PROBE_TRACE traces the random draws and $A3PROBE_SRAND fixes the seed of
 * every srand() (the game seeds with the time). The commands:
 *
 *   ping                 ok probe 1
 *   peek ADDR LEN        ok HEX         reads memory of the game
 *   poke ADDR HEX        ok             writes it
 *   windows              ok HWND CLASS X Y W H client X Y W H style S "TITLE"; ...
 *                                       the game's visible top-level windows,
 *                                       topmost first, in screen coordinates
 *   click X Y            ok             clicks the left button at X, Y of the
 *                                       game's 640×480 screen, as the mouse
 *                                       would (SendInput)
 *   rclick X Y           ok             the same with the right button
 *   drag X1 Y1 X2 Y2     ok             drags with the left button held from
 *                                       X1, Y1 to X2, Y2 of the game's screen
 *   key VK               ok             presses and releases a virtual key
 *   text TEXT            ok             types TEXT (Windows-1252) with the keys
 *                                       of the keyboard layout
 *   dialogs              ok SHOWN ID:THIS:FLAGS:PARENT:MODAL ...
 *                                       how many dialogs the game has shown,
 *                                       and those not destroyed, oldest first
 *                                       (flag 0x10: up; MODAL 1 in DoModal)
 *   grab PATH            ok W H BITS    writes the game's whole 640×480 picture
 *                                       as a 24-bit BMP, from the surface it
 *                                       composes every frame in
 *   dump PATH            ok             writes the game's state as a savegame,
 *                                       with the original's own writer, at the
 *                                       logic thread's next safe point
 *   dumps DIR|off        ok             a dump on every new day into DIR
 *   where                ok thread ID eip EIP RET ...
 *                                       where the game's logic thread is
 *   watch ADDR           ok 1           logs every write to the two bytes at
 *                                       ADDR (the page made read-only)
 *   hooks                ok NAME:STATUS ...
 *                                       the hooks (hooks.c): on, differs (the
 *                                       routine starts unexpectedly), off
 *   counts               ok SOURCE N ...  the random draws so far, by source
 *   trace PATH|off       ok             writes every draw to PATH (hooks.c)
 *   srand SEED|off       ok             every later srand() uses SEED
 *   quit                 ok             ends the game
 *
 * Numbers may be hex (0x...) or decimal. Errors answer "error WHAT".
 */
#include <winsock2.h>
#include <windows.h>
#include <stdarg.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "probe.h"

#define VERSION 1
#define DEFAULT_PORT 47300
#define MAX_PEEK 65536

static FILE *log_file;
static CRITICAL_SECTION log_lock;

void note(const char *format, ...)
{
	if (!log_file)
		return;
	EnterCriticalSection(&log_lock);
	SYSTEMTIME now;
	GetLocalTime(&now);
	fprintf(log_file, "%02d:%02d:%02d.%03d ", now.wHour, now.wMinute, now.wSecond,
		now.wMilliseconds);
	va_list args;
	va_start(args, format);
	vfprintf(log_file, format, args);
	va_end(args);
	fputc('\n', log_file);
	fflush(log_file);
	LeaveCriticalSection(&log_lock);
}

/* Whether [address, address + length) is committed memory the game may read
 * (or, with write, write). */
static BOOL usable(const void *address, SIZE_T length, BOOL write)
{
	const char *at = address;
	while (length > 0) {
		MEMORY_BASIC_INFORMATION info;
		if (!VirtualQuery(at, &info, sizeof info) || info.State != MEM_COMMIT ||
		    (info.Protect & (PAGE_NOACCESS | PAGE_GUARD)))
			return FALSE;
		DWORD writable = PAGE_READWRITE | PAGE_WRITECOPY | PAGE_EXECUTE_READWRITE |
				 PAGE_EXECUTE_WRITECOPY;
		if (write && !(info.Protect & writable))
			return FALSE;
		SIZE_T left = (const char *)info.BaseAddress + info.RegionSize - at;
		if (left >= length)
			break;
		at += left;
		length -= left;
	}
	return TRUE;
}

static int hex_digit(char c)
{
	if (c >= '0' && c <= '9')
		return c - '0';
	if (c >= 'a' && c <= 'f')
		return c - 'a' + 10;
	if (c >= 'A' && c <= 'F')
		return c - 'A' + 10;
	return -1;
}

/* The game's visible top-level windows, topmost first. */
struct windows {
	HWND list[64];
	int count;
};

static BOOL CALLBACK collect(HWND window, LPARAM found)
{
	struct windows *windows = (struct windows *)found;
	DWORD process;
	GetWindowThreadProcessId(window, &process);
	if (process == GetCurrentProcessId() && IsWindowVisible(window) &&
	    windows->count < (int)(sizeof windows->list / sizeof *windows->list))
		windows->list[windows->count++] = window;
	return TRUE;
}

static void list_windows(char *reply, size_t size)
{
	struct windows windows = {.count = 0};
	EnumWindows(collect, (LPARAM)&windows);
	size_t used = snprintf(reply, size, "ok");
	for (int i = 0; i < windows.count && used < size; i++) {
		char class[64], title[128];
		RECT r;
		GetClassNameA(windows.list[i], class, sizeof class);
		GetWindowTextA(windows.list[i], title, sizeof title);
		GetWindowRect(windows.list[i], &r);
		RECT c;
		POINT o = {0, 0};
		GetClientRect(windows.list[i], &c);
		ClientToScreen(windows.list[i], &o);
		used += snprintf(reply + used, size - used,
				 "%s %#lx %s %ld %ld %ld %ld client %ld %ld %ld %ld style %#lx \"%s\"",
				 i ? ";" : "", (unsigned long)(UINT_PTR)windows.list[i], class,
				 r.left, r.top, r.right - r.left, r.bottom - r.top, o.x, o.y, c.right,
				 c.bottom, GetWindowLongA(windows.list[i], GWL_STYLE), title);
	}
}

/* The game's main window: the bottommost of its visible top-level windows. */
static HWND main_window(void)
{
	struct windows windows = {.count = 0};
	EnumWindows(collect, (LPARAM)&windows);
	return windows.count ? windows.list[windows.count - 1] : NULL;
}

/* Input as the hardware gives it, so the game sees it however it reads it. */
static void inject(INPUT *inputs, int count)
{
	for (int i = 0; i < count; i++) {
		SendInput(1, &inputs[i], sizeof *inputs);
		Sleep(30);
	}
}

static void click(int x, int y, BOOL right, char *reply, size_t size)
{
	HWND window = main_window();
	POINT at = {x, y};
	if (!window || !ClientToScreen(window, &at)) {
		snprintf(reply, size, "error no game window");
		return;
	}
	int width = GetSystemMetrics(SM_CXSCREEN), height = GetSystemMetrics(SM_CYSCREEN);
	INPUT inputs[3] = {{.type = INPUT_MOUSE}, {.type = INPUT_MOUSE}, {.type = INPUT_MOUSE}};
	inputs[0].mi.dx = (at.x * 65535 + width / 2) / (width - 1);
	inputs[0].mi.dy = (at.y * 65535 + height / 2) / (height - 1);
	inputs[0].mi.dwFlags = MOUSEEVENTF_MOVE | MOUSEEVENTF_ABSOLUTE;
	inputs[1].mi.dwFlags = right ? MOUSEEVENTF_RIGHTDOWN : MOUSEEVENTF_LEFTDOWN;
	inputs[2].mi.dwFlags = right ? MOUSEEVENTF_RIGHTUP : MOUSEEVENTF_LEFTUP;
	inject(inputs, 3);
	snprintf(reply, size, "ok");
}

/* Takes hold at X1, Y1 of the game's screen, moves there to X2, Y2 in a few
 * steps, as a hand would, and lets go. */
#define DRAG_STEPS 6

static void drag(int x1, int y1, int x2, int y2, char *reply, size_t size)
{
	HWND window = main_window();
	POINT from = {x1, y1}, to = {x2, y2};
	if (!window || !ClientToScreen(window, &from) || !ClientToScreen(window, &to)) {
		snprintf(reply, size, "error no game window");
		return;
	}
	int width = GetSystemMetrics(SM_CXSCREEN), height = GetSystemMetrics(SM_CYSCREEN);
	INPUT inputs[DRAG_STEPS + 3] = {{0}};
	for (int i = 0; i < DRAG_STEPS + 3; i++)
		inputs[i].type = INPUT_MOUSE;
	for (int i = 0; i <= DRAG_STEPS; i++) {
		INPUT *move = &inputs[i == 0 ? 0 : i + 1];
		int x = from.x + (to.x - from.x) * i / DRAG_STEPS, y = from.y + (to.y - from.y) * i / DRAG_STEPS;
		move->mi.dx = (x * 65535 + width / 2) / (width - 1);
		move->mi.dy = (y * 65535 + height / 2) / (height - 1);
		move->mi.dwFlags = MOUSEEVENTF_MOVE | MOUSEEVENTF_ABSOLUTE;
	}
	inputs[1].mi.dwFlags = MOUSEEVENTF_LEFTDOWN;
	inputs[DRAG_STEPS + 2].mi.dwFlags = MOUSEEVENTF_LEFTUP;
	inject(inputs, DRAG_STEPS + 3);
	snprintf(reply, size, "ok");
}

/* Presses and releases a virtual key, with Shift, Ctrl and Alt as the bits
 * 1, 2, 4 of shift say. */
static void key(WORD code, int shift)
{
	static const WORD modifiers[3] = {VK_SHIFT, VK_CONTROL, VK_MENU};
	INPUT inputs[8];
	int count = 0;
	for (int i = 0; i < 3; i++) {
		if (shift & 1 << i)
			inputs[count++] = (INPUT){.type = INPUT_KEYBOARD, .ki.wVk = modifiers[i]};
	}
	inputs[count++] = (INPUT){.type = INPUT_KEYBOARD, .ki.wVk = code};
	inputs[count++] = (INPUT){.type = INPUT_KEYBOARD, .ki.wVk = code, .ki.dwFlags = KEYEVENTF_KEYUP};
	for (int i = 2; i >= 0; i--) {
		if (shift & 1 << i)
			inputs[count++] = (INPUT){.type = INPUT_KEYBOARD,
						  .ki.wVk = modifiers[i],
						  .ki.dwFlags = KEYEVENTF_KEYUP};
	}
	inject(inputs, count);
}

/* Types a Windows-1252 character with the keys the keyboard layout has for
 * it: the game reads keys, not characters. */
static BOOL type(char c)
{
	SHORT scan = VkKeyScanExA(c, GetKeyboardLayout(0));
	if (scan == -1)
		return FALSE;
	key(LOBYTE(scan), HIBYTE(scan));
	return TRUE;
}

/* The next word of the line at *cursor, or with rest everything left of it;
 * NULL at the end. Clients run in threads of their own, so no strtok. */
static char *take(char **cursor, BOOL rest)
{
	char *at = *cursor;
	while (*at == ' ' || *at == '\t')
		at++;
	if (!*at) {
		*cursor = at;
		return NULL;
	}
	char *word = at;
	if (rest) {
		at += strlen(at);
	} else {
		while (*at && *at != ' ' && *at != '\t')
			at++;
		if (*at)
			*at++ = 0;
	}
	*cursor = at;
	return word;
}

/*
 * The game's whole picture. SGL (d3d_dll.dll) composes every frame in an
 * off-screen IDirectDrawSurface4 and copies only what changed to the window,
 * so the window shows little of a screen; the render target the DLL keeps
 * (at 0x0508af80 for its preferred base 0x05000000) holds all of it, the
 * game's 640×480 at the top left.
 */
#define SGL_BASE 0x05000000u
#define SGL_RENDER_TARGET 0x0508af80u
#define FRAME_WIDTH 640
#define FRAME_HEIGHT 480
/* DDSURFACEDESC2, by offset: its size, height, width, pitch, the pixels,
 * and the pixel format's bit count and red, green and blue masks. */
#define DESC_SIZE 0x7c
#define DESC_HEIGHT 0x08
#define DESC_WIDTH 0x0c
#define DESC_PITCH 0x10
#define DESC_SURFACE 0x24
#define DESC_BITS 0x54
#define DESC_MASKS 0x58
/* IDirectDrawSurface4's Lock and Unlock in its vtable. */
#define SURFACE_LOCK 25
#define SURFACE_UNLOCK 32
#define DDLOCK_WAIT_READONLY 0x11

typedef HRESULT (WINAPI *surface_lock)(void *, RECT *, void *, DWORD, HANDLE);
typedef HRESULT (WINAPI *surface_unlock)(void *, RECT *);

/* A colour channel of a pixel, scaled to 8 bits. */
static BYTE channel(DWORD pixel, DWORD mask)
{
	if (!mask)
		return 0;
	int shift = 0, width = 0;
	while (!(mask >> shift & 1))
		shift++;
	while (shift + width < 32 && mask >> (shift + width) & 1)
		width++;
	DWORD value = (pixel & mask) >> shift;
	return (BYTE)(width >= 8 ? value >> (width - 8) : value * 255 / ((1u << width) - 1));
}

static void put32(BYTE *at, DWORD value)
{
	memcpy(at, &value, 4);
}

static void grab(const char *path, char *reply, size_t size)
{
	HMODULE sgl = GetModuleHandleA("d3d_dll.dll");
	void **target = sgl ? (void **)((BYTE *)sgl + (SGL_RENDER_TARGET - SGL_BASE)) : NULL;
	void *surface = target && usable(target, sizeof *target, FALSE) ? *target : NULL;
	if (!surface) {
		snprintf(reply, size, "error no picture yet (d3d_dll.dll %s)", sgl ? "has none" : "not loaded");
		return;
	}
	void **vtable = *(void ***)surface;
	BYTE desc[DESC_SIZE] = {0};
	put32(desc, DESC_SIZE);
	HRESULT locked = ((surface_lock)vtable[SURFACE_LOCK])(surface, NULL, desc, DDLOCK_WAIT_READONLY, NULL);
	if (FAILED(locked)) {
		snprintf(reply, size, "error the surface does not lock: %08lx", (unsigned long)locked);
		return;
	}
	DWORD height, width, bits, masks[3];
	LONG pitch;
	BYTE *pixels;
	memcpy(&height, desc + DESC_HEIGHT, 4);
	memcpy(&width, desc + DESC_WIDTH, 4);
	memcpy(&pitch, desc + DESC_PITCH, 4);
	memcpy(&pixels, desc + DESC_SURFACE, sizeof pixels);
	memcpy(&bits, desc + DESC_BITS, 4);
	memcpy(masks, desc + DESC_MASKS, sizeof masks);
	/* A BMP: bottom-up rows of blue, green, red; 640 × 3 needs no padding. */
	static BYTE image[54 + FRAME_WIDTH * FRAME_HEIGHT * 3];
	BOOL fits = width >= FRAME_WIDTH && height >= FRAME_HEIGHT && (bits == 16 || bits == 24 || bits == 32);
	if (fits) {
		for (int y = 0; y < FRAME_HEIGHT; y++) {
			BYTE *row = pixels + (size_t)y * pitch;
			BYTE *out = image + 54 + (size_t)(FRAME_HEIGHT - 1 - y) * FRAME_WIDTH * 3;
			for (int x = 0; x < FRAME_WIDTH; x++) {
				DWORD pixel = 0;
				memcpy(&pixel, row + x * (bits / 8), bits / 8);
				out[3 * x] = channel(pixel, masks[2]);
				out[3 * x + 1] = channel(pixel, masks[1]);
				out[3 * x + 2] = channel(pixel, masks[0]);
			}
		}
	}
	((surface_unlock)vtable[SURFACE_UNLOCK])(surface, NULL);
	if (!fits) {
		snprintf(reply, size, "error a picture of %lux%lu at %lu bits", (unsigned long)width,
			 (unsigned long)height, (unsigned long)bits);
		return;
	}
	memset(image, 0, 54);
	image[0] = 'B';
	image[1] = 'M';
	put32(image + 2, sizeof image);
	put32(image + 10, 54);
	put32(image + 14, 40);
	put32(image + 18, FRAME_WIDTH);
	put32(image + 22, FRAME_HEIGHT);
	image[26] = 1;
	image[28] = 24;
	put32(image + 34, FRAME_WIDTH * FRAME_HEIGHT * 3);
	FILE *file = fopen(path, "wb");
	if (!file || fwrite(image, sizeof image, 1, file) != 1) {
		snprintf(reply, size, "error cannot write %s", path);
	} else {
		snprintf(reply, size, "ok %lu %lu %lu", (unsigned long)width, (unsigned long)height,
			 (unsigned long)bits);
	}
	if (file)
		fclose(file);
}

/* Answers one command line into reply; whether the game ends after it. */
static BOOL answer(char *line, char *reply, size_t size)
{
	char *cursor = line;
	char *word = take(&cursor, FALSE);
	if (!word) {
		snprintf(reply, size, "error empty command");
	} else if (!strcmp(word, "ping")) {
		snprintf(reply, size, "ok probe %d", VERSION);
	} else if (!strcmp(word, "peek")) {
		char *address = take(&cursor, FALSE), *count = take(&cursor, FALSE);
		unsigned long at = address ? strtoul(address, NULL, 0) : 0;
		unsigned long length = count ? strtoul(count, NULL, 0) : 0;
		if (!at || !length || length > MAX_PEEK || 4 + 2 * length >= size) {
			snprintf(reply, size, "error peek ADDR LEN, at most %d bytes", MAX_PEEK);
		} else if (!usable((void *)at, length, FALSE)) {
			snprintf(reply, size, "error %#lx is not readable", at);
		} else {
			char *out = reply + sprintf(reply, "ok ");
			for (unsigned long i = 0; i < length; i++)
				out += sprintf(out, "%02x", ((unsigned char *)at)[i]);
		}
	} else if (!strcmp(word, "poke")) {
		char *address = take(&cursor, FALSE), *bytes = take(&cursor, FALSE);
		unsigned long at = address ? strtoul(address, NULL, 0) : 0;
		size_t length = bytes ? strlen(bytes) / 2 : 0;
		if (!at || !length || strlen(bytes) % 2) {
			snprintf(reply, size, "error poke ADDR HEX");
		} else if (!usable((void *)at, length, TRUE)) {
			snprintf(reply, size, "error %#lx is not writable", at);
		} else {
			for (size_t i = 0; i < length; i++) {
				int high = hex_digit(bytes[2 * i]), low = hex_digit(bytes[2 * i + 1]);
				if (high < 0 || low < 0) {
					snprintf(reply, size, "error not hex: %s", bytes);
					return FALSE;
				}
				((unsigned char *)at)[i] = (unsigned char)(high << 4 | low);
			}
			snprintf(reply, size, "ok");
		}
	} else if (!strcmp(word, "windows")) {
		list_windows(reply, size);
	} else if (!strcmp(word, "click") || !strcmp(word, "rclick")) {
		char *x = take(&cursor, FALSE), *y = take(&cursor, FALSE);
		if (!x || !y)
			snprintf(reply, size, "error %s X Y", word);
		else
			click(atoi(x), atoi(y), word[0] == 'r', reply, size);
	} else if (!strcmp(word, "drag")) {
		char *x1 = take(&cursor, FALSE), *y1 = take(&cursor, FALSE);
		char *x2 = take(&cursor, FALSE), *y2 = take(&cursor, FALSE);
		if (!x1 || !y1 || !x2 || !y2)
			snprintf(reply, size, "error drag X1 Y1 X2 Y2");
		else
			drag(atoi(x1), atoi(y1), atoi(x2), atoi(y2), reply, size);
	} else if (!strcmp(word, "key")) {
		char *code = take(&cursor, FALSE);
		if (!code) {
			snprintf(reply, size, "error key VK");
		} else {
			key((WORD)strtoul(code, NULL, 0), 0);
			snprintf(reply, size, "ok");
		}
	} else if (!strcmp(word, "text")) {
		char *text = take(&cursor, TRUE);
		if (!text) {
			snprintf(reply, size, "error text TEXT");
		} else {
			/* The client sends Windows-1252. */
			char *c = text;
			while (*c && type(*c))
				c++;
			if (*c)
				snprintf(reply, size, "error no key for %#x", (unsigned char)*c);
			else
				snprintf(reply, size, "ok");
		}
	} else if (!strcmp(word, "grab")) {
		char *path = take(&cursor, TRUE);
		if (!path)
			snprintf(reply, size, "error grab PATH");
		else
			grab(path, reply, size);
	} else if (!strcmp(word, "dump")) {
		char *path = take(&cursor, TRUE);
		if (!path)
			snprintf(reply, size, "error dump PATH");
		else if (dump(path, 20000)) /* within the client's 30 s */
			snprintf(reply, size, "error no safe point came for the dump");
		else
			snprintf(reply, size, "ok");
	} else if (!strcmp(word, "dumps")) {
		char *dir = take(&cursor, TRUE);
		if (!dir) {
			snprintf(reply, size, "error dumps DIR|off");
		} else {
			dump_daily(strcmp(dir, "off") ? dir : NULL);
			snprintf(reply, size, "ok");
		}
	} else if (!strcmp(word, "watch")) {
		char *address = take(&cursor, FALSE);
		if (!address)
			snprintf(reply, size, "error watch ADDR");
		else
			snprintf(reply, size, "ok %d", watch((DWORD)strtoul(address, NULL, 0)));
	} else if (!strcmp(word, "where")) {
		report_logic_thread(reply, size);
	} else if (!strcmp(word, "dialogs")) {
		report_dialogs(reply, size);
	} else if (!strcmp(word, "hooks")) {
		report_hooks(reply, size);
	} else if (!strcmp(word, "counts")) {
		report_counts(reply, size);
	} else if (!strcmp(word, "trace")) {
		char *path = take(&cursor, TRUE);
		if (!path)
			snprintf(reply, size, "error trace PATH|off");
		else if (trace_draws(strcmp(path, "off") ? path : NULL))
			snprintf(reply, size, "error cannot write %s", path);
		else
			snprintf(reply, size, "ok");
	} else if (!strcmp(word, "srand")) {
		char *seed = take(&cursor, FALSE);
		if (!seed)
			snprintf(reply, size, "error srand SEED|off");
		else {
			fix_crt_seed(strcmp(seed, "off") != 0, (unsigned)strtoul(seed, NULL, 0));
			snprintf(reply, size, "ok");
		}
	} else if (!strcmp(word, "quit")) {
		snprintf(reply, size, "ok");
		return TRUE;
	} else {
		snprintf(reply, size, "error unknown command %s", word);
	}
	return FALSE;
}

#define LINE 4096
#define REPLY (2 * MAX_PEEK + 64)

static void serve_client(SOCKET client, char *line, char *reply)
{
	size_t used = 0;
	for (;;) {
		int got = recv(client, line + used, (int)(LINE - 1 - used), 0);
		if (got <= 0)
			return;
		used += got;
		char *end;
		while ((end = memchr(line, '\n', used))) {
			*end = 0;
			if (end > line && end[-1] == '\r')
				end[-1] = 0;
			note("< %s", line);
			BOOL quit = answer(line, reply, REPLY);
			note("> %.120s", reply);
			strcat(reply, "\n");
			send(client, reply, (int)strlen(reply), 0);
			if (quit) {
				shutdown(client, SD_SEND);
				note("quit");
				trace_draws(NULL);
				ExitProcess(0);
			}
			used -= end + 1 - line;
			memmove(line, end + 1, used);
		}
		if (used == LINE - 1)
			used = 0; /* a line too long to answer */
	}
}

/* A client of its own thread: a script runner and a watcher may be connected
 * at once. */
static DWORD WINAPI serve_one(LPVOID socket)
{
	SOCKET client = (SOCKET)socket;
	char *line = malloc(LINE), *reply = malloc(REPLY);
	note("client connected");
	if (line && reply)
		serve_client(client, line, reply);
	closesocket(client);
	note("client gone");
	free(line);
	free(reply);
	return 0;
}

static DWORD WINAPI serve(LPVOID unused)
{
	(void)unused;
	WSADATA wsa;
	if (WSAStartup(MAKEWORD(2, 2), &wsa)) {
		note("WSAStartup failed");
		return 1;
	}
	char value[16];
	int port = GetEnvironmentVariableA("A3PROBE_PORT", value, sizeof value) ? atoi(value)
										 : DEFAULT_PORT;
	SOCKET server = socket(AF_INET, SOCK_STREAM, IPPROTO_TCP);
	struct sockaddr_in local = {.sin_family = AF_INET, .sin_port = htons((u_short)port)};
	local.sin_addr.s_addr = htonl(INADDR_LOOPBACK);
	BOOL reuse = TRUE;
	setsockopt(server, SOL_SOCKET, SO_REUSEADDR, (const char *)&reuse, sizeof reuse);
	if (bind(server, (struct sockaddr *)&local, sizeof local) || listen(server, 8)) {
		note("cannot listen on port %d (error %d)", port, WSAGetLastError());
		return 1;
	}
	note("listening on 127.0.0.1:%d", port);
	for (;;) {
		SOCKET client = accept(server, NULL, NULL);
		if (client == INVALID_SOCKET)
			continue;
		HANDLE thread = CreateThread(NULL, 0, serve_one, (LPVOID)client, 0, NULL);
		if (thread)
			CloseHandle(thread);
		else
			closesocket(client);
	}
}

BOOL WINAPI DllMain(HINSTANCE self, DWORD reason, LPVOID reserved)
{
	(void)reserved;
	if (reason != DLL_PROCESS_ATTACH)
		return TRUE;
	DisableThreadLibraryCalls(self);
	InitializeCriticalSection(&log_lock);
	char path[MAX_PATH];
	if (!GetEnvironmentVariableA("A3PROBE_LOG", path, sizeof path))
		strcpy(path, "probe.log");
	log_file = fopen(path, "a");
	note("probe %d in process %lu", VERSION, GetCurrentProcessId());
	install_hooks();
	char trace[MAX_PATH], seed[16];
	if (GetEnvironmentVariableA("A3PROBE_TRACE", trace, sizeof trace) && trace_draws(trace))
		note("cannot write the trace %s", trace);
	if (GetEnvironmentVariableA("A3PROBE_SRAND", seed, sizeof seed))
		fix_crt_seed(1, (unsigned)strtoul(seed, NULL, 0));
	HANDLE thread = CreateThread(NULL, 0, serve, NULL, 0, NULL);
	if (thread)
		CloseHandle(thread);
	return TRUE;
}
