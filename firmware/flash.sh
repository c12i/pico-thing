#!/bin/sh
# Flash firmware/pico-thing.uf2: reboot the Pico into its USB bootloader (no BOOTSEL
# button needed), copy the UF2, then put pico/main.py and pico/config.py back and
# remove any filesystem .mpy/.py modules that would shadow the frozen ones.
set -e
cd "$(dirname "$0")"
ROOT=$(cd .. && pwd)
MPREMOTE=$(command -v mpremote || echo "$HOME/.local/bin/mpremote")
UF2=pico-thing.uf2
[ -f "$UF2" ] || { echo "no $UF2 - run ./build.sh first"; exit 1; }

if [ ! -d /Volumes/RPI-RP2 ]; then
    "$MPREMOTE" exec "import machine; machine.bootloader()" >/dev/null 2>&1 || true
    for _ in $(seq 1 40); do [ -d /Volumes/RPI-RP2 ] && break; sleep 0.5; done
fi
[ -d /Volumes/RPI-RP2 ] || { echo "Pico bootloader drive didn't appear"; exit 1; }
# Writes fail with "Permission denied" for a moment right after the drive mounts
for _ in 1 2 3 4 5; do
    sleep 1
    cp "$UF2" /Volumes/RPI-RP2/ 2>/dev/null && break
done
echo "flashed, waiting for the Pico to restart..."
for _ in $(seq 1 60); do ls /dev/cu.usbmodem* >/dev/null 2>&1 && break; sleep 0.5; done
sleep 2

STALE=$(for f in "$ROOT"/pico/*/*.py; do b=$(basename "$f" .py); printf '"%s.py","%s.mpy",' "$b" "$b"; done)
"$MPREMOTE" exec "import os
for f in ($STALE):
    try:
        os.remove(f)
    except OSError:
        pass" + cp "$ROOT/pico/main.py" "$ROOT/pico/config.py" : + reset
echo done
