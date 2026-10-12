/*
 * The probe inside the original: loaded by launch.exe before the game runs,
 * it keeps a log and answers commands, one line each, on a TCP socket on
 * localhost — port $A3PROBE_PORT, 47300 by default. The commands:
 *
 *   ping                 ok probe 1
 *   peek ADDR LEN        ok HEX         reads memory of the game
 *   poke ADDR HEX        ok             writes it
 *   windows              ok HWND CLASS X Y W H "TITLE"; ...
 *                                       the game's visible top-level windows,
 *                                       topmost first
 *   capture PATH         ok             saves the game's screen, 640×480 from
 *                                       the top left, as its windows draw it,
 *                                       as a 24-bit BMP
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

#define VERSION 1
#define DEFAULT_PORT 47300
#define MAX_PEEK 65536

static FILE *log_file;
static CRITICAL_SECTION log_lock;

static void note(const char *format, ...)
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

/* Draws the game's windows bottom up into a bitmap of the game's screen and
 * writes it to path. The menus blit to the top left of the screen, wherever
 * the main window is. */
static void capture(const char *path, char *reply, size_t size)
{
	int width = 640, height = 480;
	struct windows windows = {.count = 0};
	EnumWindows(collect, (LPARAM)&windows);
	BITMAPINFOHEADER header = {
		.biSize = sizeof header,
		.biWidth = width,
		.biHeight = height,
		.biPlanes = 1,
		.biBitCount = 24,
		.biCompression = BI_RGB,
	};
	void *pixels;
	HDC screen = GetDC(NULL), canvas = CreateCompatibleDC(screen);
	HBITMAP bitmap = CreateDIBSection(screen, (BITMAPINFO *)&header, DIB_RGB_COLORS, &pixels,
					  NULL, 0);
	if (!bitmap) {
		snprintf(reply, size, "error cannot make a %dx%d bitmap", width, height);
		DeleteDC(canvas);
		ReleaseDC(NULL, screen);
		return;
	}
	HGDIOBJ old = SelectObject(canvas, bitmap);
	PatBlt(canvas, 0, 0, width, height, BLACKNESS);
	for (int i = windows.count - 1; i >= 0; i--) {
		RECT r;
		GetWindowRect(windows.list[i], &r);
		HDC from = GetWindowDC(windows.list[i]);
		BitBlt(canvas, r.left, r.top, r.right - r.left, r.bottom - r.top, from, 0, 0, SRCCOPY);
		ReleaseDC(windows.list[i], from);
	}
	GdiFlush();
	DWORD stride = (width * 3 + 3) & ~3u, image = stride * height;
	BITMAPFILEHEADER file = {
		.bfType = 0x4d42,
		.bfSize = sizeof file + sizeof header + image,
		.bfOffBits = sizeof file + sizeof header,
	};
	FILE *out = fopen(path, "wb");
	if (!out) {
		snprintf(reply, size, "error cannot write %s", path);
	} else {
		fwrite(&file, sizeof file, 1, out);
		fwrite(&header, sizeof header, 1, out);
		fwrite(pixels, image, 1, out);
		fclose(out);
		snprintf(reply, size, "ok");
	}
	SelectObject(canvas, old);
	DeleteObject(bitmap);
	DeleteDC(canvas);
	ReleaseDC(NULL, screen);
}

/* Answers one command line into reply; whether the game ends after it. */
static BOOL answer(char *line, char *reply, size_t size)
{
	char *word = strtok(line, " \t");
	if (!word) {
		snprintf(reply, size, "error empty command");
	} else if (!strcmp(word, "ping")) {
		snprintf(reply, size, "ok probe %d", VERSION);
	} else if (!strcmp(word, "peek")) {
		char *address = strtok(NULL, " \t"), *count = strtok(NULL, " \t");
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
		char *address = strtok(NULL, " \t"), *bytes = strtok(NULL, " \t");
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
	} else if (!strcmp(word, "capture")) {
		char *path = strtok(NULL, "");
		if (!path)
			snprintf(reply, size, "error capture PATH");
		else
			capture(path, reply, size);
	} else if (!strcmp(word, "quit")) {
		snprintf(reply, size, "ok");
		return TRUE;
	} else {
		snprintf(reply, size, "error unknown command %s", word);
	}
	return FALSE;
}

static void serve_client(SOCKET client)
{
	static char line[4096], reply[2 * MAX_PEEK + 64];
	size_t used = 0;
	for (;;) {
		int got = recv(client, line + used, (int)(sizeof line - 1 - used), 0);
		if (got <= 0)
			return;
		used += got;
		char *end;
		while ((end = memchr(line, '\n', used))) {
			*end = 0;
			if (end > line && end[-1] == '\r')
				end[-1] = 0;
			note("< %s", line);
			BOOL quit = answer(line, reply, sizeof reply);
			note("> %.120s", reply);
			strcat(reply, "\n");
			send(client, reply, (int)strlen(reply), 0);
			if (quit) {
				shutdown(client, SD_SEND);
				note("quit");
				ExitProcess(0);
			}
			used -= end + 1 - line;
			memmove(line, end + 1, used);
		}
		if (used == sizeof line - 1)
			used = 0; /* a line too long to answer */
	}
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
	if (bind(server, (struct sockaddr *)&local, sizeof local) || listen(server, 1)) {
		note("cannot listen on port %d (error %d)", port, WSAGetLastError());
		return 1;
	}
	note("listening on 127.0.0.1:%d", port);
	for (;;) {
		SOCKET client = accept(server, NULL, NULL);
		if (client == INVALID_SOCKET)
			continue;
		note("client connected");
		serve_client(client);
		closesocket(client);
		note("client gone");
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
	HANDLE thread = CreateThread(NULL, 0, serve, NULL, 0, NULL);
	if (thread)
		CloseHandle(thread);
	return TRUE;
}
