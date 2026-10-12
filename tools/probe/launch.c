/*
 * Starts the original with the probe in it: anstoss3.exe (or the program
 * named first) suspended, probe.dll from beside the launcher loaded into it
 * through a thread of its own, then on its way. The launcher waits for the
 * game and ends with its exit code.
 */
#include <windows.h>
#include <stdio.h>
#include <string.h>

static int fail(const char *what)
{
	fprintf(stderr, "launch: %s failed (error %lu)\n", what, GetLastError());
	return 1;
}

int main(int argc, char **argv)
{
	const char *program = argc > 1 ? argv[1] : "anstoss3.exe";
	char dll[MAX_PATH];
	DWORD length = GetModuleFileNameA(NULL, dll, MAX_PATH);
	char *slash = strrchr(dll, '\\');
	if (length == 0 || !slash || slash - dll + sizeof "\\probe.dll" > MAX_PATH)
		return fail("finding probe.dll");
	strcpy(slash + 1, "probe.dll");

	STARTUPINFOA startup = {.cb = sizeof startup};
	PROCESS_INFORMATION game;
	if (!CreateProcessA(program, NULL, NULL, NULL, FALSE, CREATE_SUSPENDED, NULL, NULL,
			    &startup, &game))
		return fail("starting the game");

	SIZE_T size = strlen(dll) + 1;
	void *path = VirtualAllocEx(game.hProcess, NULL, size, MEM_COMMIT | MEM_RESERVE,
				    PAGE_READWRITE);
	if (!path || !WriteProcessMemory(game.hProcess, path, dll, size, NULL)) {
		TerminateProcess(game.hProcess, 1);
		return fail("writing the probe's path");
	}
	/* kernel32 lies at the same address in both processes. */
	FARPROC load = GetProcAddress(GetModuleHandleA("kernel32.dll"), "LoadLibraryA");
	HANDLE loader = CreateRemoteThread(game.hProcess, NULL, 0, (LPTHREAD_START_ROUTINE)load,
					   path, 0, NULL);
	DWORD module = 0;
	if (!loader || WaitForSingleObject(loader, 30000) != WAIT_OBJECT_0 ||
	    !GetExitCodeThread(loader, &module) || module == 0) {
		TerminateProcess(game.hProcess, 1);
		return fail("loading the probe");
	}
	CloseHandle(loader);
	VirtualFreeEx(game.hProcess, path, 0, MEM_RELEASE);
	fprintf(stderr, "launch: probe loaded at %#lx\n", module);

	ResumeThread(game.hThread);
	WaitForSingleObject(game.hProcess, INFINITE);
	DWORD code = 0;
	GetExitCodeProcess(game.hProcess, &code);
	return (int)code;
}
