# The probe and its launcher, for the 32-bit original under Wine.
#
#   make            # builds into target/probe/ of the replica

CC = i686-w64-mingw32-gcc
CFLAGS = -O2 -Wall -Wextra -std=c11 -static-libgcc
OUT ?= ../../target/probe

all: $(OUT)/probe.dll $(OUT)/launch.exe

$(OUT)/probe.dll: probe.c | $(OUT)
	$(CC) $(CFLAGS) -shared -o $@ $< -lws2_32 -luser32

$(OUT)/launch.exe: launch.c | $(OUT)
	$(CC) $(CFLAGS) -o $@ $<

$(OUT):
	mkdir -p $@

clean:
	rm -f $(OUT)/probe.dll $(OUT)/launch.exe

.PHONY: all clean
